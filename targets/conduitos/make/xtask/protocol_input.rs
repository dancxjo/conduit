//! Prepare canonical Fore input with the checked schema, never device knowledge.
use super::ConduitosError;
use crate::cli::GlobalOpts;
use clap::Args;
use conduit_core::{
    FrontValueLocation, PortId, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape as Shape, StructuredInfoValue,
};
use conduitos::{
    protocol_boot::ProtocolBootInput,
    protocol_source::{PreparedProtocolEntry, MAXIMUM_PACKAGE_BYTES},
};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Args, Debug)]
pub(super) struct InputArgs {
    #[arg(long)]
    package: PathBuf,
    #[arg(long)]
    entry: String,
    #[arg(long)]
    port: String,
    /// Typed JSON: record objects, sequence arrays, single-key variant objects;
    /// scalar leaves are arrays of their canonical bytes.
    #[arg(long)]
    value: PathBuf,
    /// New JSON file containing one Root request input item.
    #[arg(long)]
    output: PathBuf,
}

pub(super) fn execute(args: InputArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!(
            "Encode checked {} input {} into {}",
            args.entry,
            args.port,
            args.output.display()
        );
        return Ok(());
    }
    let package = read(&args.package, MAXIMUM_PACKAGE_BYTES)?;
    let entry = PreparedProtocolEntry::prepare(&package, &args.entry)
        .map_err(|error| refusal("protocol-input-source-refused", format!("{error:?}")))?;
    let port = PortId::from(args.port.clone());
    let schema = entry
        .input_schema(&port)
        .ok_or_else(|| refusal("protocol-input-port-refused", &args.port))?;
    let json = serde_json::from_slice(&read(&args.value, 65536)?)
        .map_err(|error| refusal("protocol-input-json-refused", error))?;
    let value = encode(&schema, &json, 0, &mut 4096)
        .map_err(|error| refusal("protocol-input-value-refused", format!("{error:?}")))?;
    let expected_kind = &entry
        .expanded()
        .front
        .inputs()
        .iter()
        .find(|input| input.port_id == port)
        .ok_or_else(|| refusal("protocol-input-port-refused", "port absent"))?
        .value_kind;
    let contract = entry
        .expanded()
        .front
        .value_contract(&FrontValueLocation::Input(port));
    let canonical_bytes = match schema.shape() {
        Shape::Leaf(kind) if kind == expected_kind => match value.shape() {
            conduit_core::StructuredInfoValueShape::Leaf(bytes) => bytes.to_vec(),
            _ => {
                return Err(refusal(
                    "protocol-input-kind-refused",
                    "non-leaf primitive input",
                ))
            }
        },
        _ => {
            if value
                .value_type()
                .profile()
                .map_err(|error| refusal("protocol-input-kind-refused", format!("{error:?}")))?
                .value_kind()
                != expected_kind
            {
                return Err(refusal(
                    "protocol-input-kind-refused",
                    "checked Fore and value schema differ",
                ));
            }
            value
                .canonical_bytes()
                .map_err(|error| refusal("protocol-input-encoding-refused", format!("{error:?}")))?
        }
    };
    if let Some(contract) = contract {
        contract
            .validate(&canonical_bytes)
            .map_err(|error| refusal("protocol-input-contract-refused", format!("{error:?}")))?;
    }
    if canonical_bytes.len() > conduitos::protocol_source::MAXIMUM_PROTOCOL_FORE_BYTES as usize {
        return Err(refusal(
            "protocol-input-envelope-exceeded",
            canonical_bytes.len(),
        ));
    }
    let bytes = serde_json::to_vec_pretty(&ProtocolBootInput {
        port: args.port,
        canonical_bytes,
    })
    .map_err(|error| refusal("protocol-input-record-refused", error))?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|error| refusal("protocol-input-output-refused", error))?;
    Ok(())
}

fn encode(
    schema: &StructuredInfoType,
    json: &Value,
    depth: u32,
    remaining: &mut usize,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    use StructuredInfoRefusal as Error;
    if depth >= 32 || *remaining == 0 {
        return Err(Error::WrongType);
    }
    *remaining -= 1;
    let nested = depth + 1;
    match schema.shape() {
        Shape::Leaf(_) => {
            let values = json.as_array().ok_or(Error::WrongType)?;
            if values.len() > 4096 {
                return Err(Error::LeafTooLarge);
            }
            let bytes = values
                .iter()
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or(Error::WrongType)
                })
                .collect::<Result<Vec<_>, _>>()?;
            StructuredInfoValue::leaf(schema.clone(), bytes)
        }
        Shape::Nominal { representation, .. } => StructuredInfoValue::nominal(
            schema.clone(),
            encode(representation, json, nested, remaining)?,
        ),
        Shape::Record { fields, .. } => {
            let object = json.as_object().ok_or(Error::WrongRecordFields)?;
            if object.len() != fields.len() {
                return Err(Error::WrongRecordFields);
            }
            let values = fields
                .iter()
                .map(|field| {
                    let json = object.get(field.name()).ok_or(Error::WrongRecordFields)?;
                    StructuredFieldValue::new(
                        field.name(),
                        encode(field.value_type(), json, nested, remaining)?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            StructuredInfoValue::record(schema.clone(), values)
        }
        Shape::Variant { cases, .. } => {
            let object = json.as_object().ok_or(Error::UnknownVariantTag)?;
            if object.len() != 1 {
                return Err(Error::UnknownVariantTag);
            }
            let (tag, payload) = object.iter().next().ok_or(Error::UnknownVariantTag)?;
            let case = cases
                .iter()
                .find(|case| case.tag() == tag)
                .ok_or(Error::UnknownVariantTag)?;
            StructuredInfoValue::variant(
                schema.clone(),
                tag,
                encode(case.payload_type(), payload, nested, remaining)?,
            )
        }
        Shape::Collection { element, length } => {
            let values = json.as_array().ok_or(Error::WrongCollectionLength)?;
            if values.len() != usize::from(length) {
                return Err(Error::WrongCollectionLength);
            }
            let values = values
                .iter()
                .map(|value| encode(element, value, nested, remaining))
                .collect::<Result<Vec<_>, _>>()?;
            StructuredInfoValue::collection(schema.clone(), values)
        }
        Shape::Sequence {
            element,
            minimum_items,
            maximum_items,
        } => {
            let values = json.as_array().ok_or(Error::WrongCollectionLength)?;
            if values.len() < usize::from(minimum_items)
                || values.len() > usize::from(maximum_items)
            {
                return Err(Error::WrongCollectionLength);
            }
            let values = values
                .iter()
                .map(|value| encode(element, value, nested, remaining))
                .collect::<Result<Vec<_>, _>>()?;
            StructuredInfoValue::sequence(schema.clone(), values)
        }
    }
}
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, ConduitosError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| refusal("protocol-input-read-refused", error))?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| refusal("protocol-input-read-refused", error))?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(refusal("protocol-input-bounds", path.display()));
    }
    Ok(bytes)
}
fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests;
