//! Native Form codecs are subordinate to checked semantic Type identities.
use super::generate::{rust_pascal_identifier, rust_snake_identifier, RustBindingGenerationError};
use crate::{CheckedTypeForm, CheckedTypeFormStorage};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::String,
};
use core::fmt::Write;

fn emit_quantity(
    out: &mut String,
    form: &CheckedTypeForm,
    repr: conduit_core::DecimalQuantityForm,
) -> Result<(), RustBindingGenerationError> {
    let name = rust_pascal_identifier(&form.name.replace('/', "-"))?;
    let integer = match repr.integer {
        conduit_core::QuantityFormInteger::I16Le => "I16Le",
        conduit_core::QuantityFormInteger::U16Le => "U16Le",
    };
    let role = match repr.role {
        conduit_core::QuantityRole::Point => "Point",
        conduit_core::QuantityRole::Delta => "Delta",
        conduit_core::QuantityRole::Linear => "Linear",
    };
    writeln!(out, "pub struct {name}Form;\nimpl {name}Form {{").unwrap();
    writeln!(out, "pub const NAME: &str = {:?};\npub const IDENTITY: &str = {:?};\npub const VALUE_TYPE: &str = {:?};\npub const EXACT_BYTES: usize = 2;",form.name,form.compatibility_id,form.value_type.as_str()).unwrap();
    writeln!(out, "pub fn definition() -> Result<conduit_core::DecimalQuantityForm, conduit_core::QuantityFormRefusal> {{ Ok(conduit_core::DecimalQuantityForm {{ integer: conduit_core::QuantityFormInteger::{integer}, unit: conduit_core::Unit::decode(&{:?}).map_err(|_| conduit_core::QuantityFormRefusal::InvalidDefinition)?, exponent: {}, role: conduit_core::QuantityRole::{role} }}) }}", repr.unit.encode(), repr.exponent).unwrap();
    out.push_str("pub fn encode(value: conduit_core::Quantity) -> Result<[u8; 2], conduit_core::QuantityFormRefusal> { Self::definition()?.encode(value) }\npub fn decode(bytes: &[u8]) -> Result<conduit_core::Quantity, conduit_core::QuantityFormRefusal> { Self::definition()?.decode(bytes) }\n}\n");
    Ok(())
}
pub(super) fn emit_forms(
    out: &mut String,
    type_forms: &[CheckedTypeForm],
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    let mut codec_names = BTreeSet::new();
    for form in type_forms {
        if let CheckedTypeFormStorage::DecimalQuantity(repr) = &form.storage {
            let codec = format!(
                "{}Form",
                rust_pascal_identifier(&form.name.replace('/', "-"))?
            );
            if !codec_names.insert(codec.clone()) {
                return Err(RustBindingGenerationError::DuplicateRustIdentifier(codec));
            }
            emit_quantity(out, form, **repr)?;
            continue;
        }
        let rust_type = names
            .get(form.value_type.as_str())
            .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
        let codec = format!("{rust_type}Form");
        if !codec_names.insert(codec.clone()) {
            return Err(RustBindingGenerationError::DuplicateRustIdentifier(codec));
        }
        if let CheckedTypeFormStorage::BooleanBits(mask) = form.storage {
            writeln!(out, "pub struct {codec};\nimpl {codec} {{").unwrap();
            writeln!(out, "pub const NAME: &str = {:?};\npub const IDENTITY: &str = {:?};\npub const EXACT_BYTES: usize = 1;", form.name, form.compatibility_id).unwrap();
            writeln!(
                out,
                "pub fn encode(value: {rust_type}) -> [u8; 1] {{ let mut byte = 0u8;"
            )
            .unwrap();
            for field in &form.mappings {
                writeln!(
                    out,
                    "if value.{} {{ byte |= {}; }}",
                    rust_snake_identifier(&field.variant)?,
                    field.discriminant
                )
                .unwrap();
            }
            writeln!(out, "[byte] }}\npub fn decode(bytes: &[u8]) -> Result<{rust_type}, NativeFormRefusal> {{\nlet [byte] = bytes else {{ return Err(NativeFormRefusal::WrongLength {{ actual: bytes.len() }}); }};\nif byte & !{mask}u8 != 0 {{ return Err(NativeFormRefusal::ReservedBits {{ actual: *byte }}); }}\nOk({rust_type} {{").unwrap();
            for field in &form.mappings {
                writeln!(
                    out,
                    "{}: byte & {} != 0,",
                    rust_snake_identifier(&field.variant)?,
                    field.discriminant
                )
                .unwrap();
            }
            out.push_str("})\n}\n}\n");
            continue;
        }
        writeln!(out, "pub struct {codec};").expect("String writing is infallible");
        writeln!(out, "impl {codec} {{").expect("String writing is infallible");
        writeln!(out, "    pub const NAME: &str = {:?};", form.name)
            .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const IDENTITY: &str = {:?};",
            form.compatibility_id
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const EXACT_BYTES: usize = {};",
            form.exact_bytes
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const MAXIMUM_BYTES: usize = {};",
            form.maximum_bytes
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const MAXIMUM_DECODE_STEPS: usize = {};",
            form.maximum_decode_steps
        )
        .expect("String writing is infallible");
        writeln!(
            out,
            "    pub const fn encode(value: {rust_type}) -> [u8; 1] {{"
        )
        .expect("String writing is infallible");
        writeln!(out, "        [match value {{").expect("String writing is infallible");
        for mapping in &form.mappings {
            writeln!(
                out,
                "            {rust_type}::{} => {},",
                rust_pascal_identifier(&mapping.variant)?,
                mapping.discriminant
            )
            .expect("String writing is infallible");
        }
        writeln!(out, "        }}]").expect("String writing is infallible");
        writeln!(out, "    }}").expect("String writing is infallible");
        writeln!(
            out,
            "    pub fn decode(encoded: &[u8]) -> Result<{rust_type}, NativeFormRefusal> {{"
        )
        .expect("String writing is infallible");
        writeln!(out, "        let [tag] = encoded else {{ return Err(NativeFormRefusal::WrongLength {{ actual: encoded.len() }}); }};")
            .expect("String writing is infallible");
        writeln!(out, "        match *tag {{").expect("String writing is infallible");
        for mapping in &form.mappings {
            writeln!(
                out,
                "            {} => Ok({rust_type}::{}),",
                mapping.discriminant,
                rust_pascal_identifier(&mapping.variant)?
            )
            .expect("String writing is infallible");
        }
        writeln!(
            out,
            "            actual => Err(NativeFormRefusal::InvalidTag {{ actual }}),"
        )
        .expect("String writing is infallible");
        writeln!(out, "        }}").expect("String writing is infallible");
        writeln!(out, "    }}\n}}\n").expect("String writing is infallible");
    }
    Ok(())
}
