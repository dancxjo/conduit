//! Opt-in appended storage bindings; ordinary generated code is unchanged.
use super::generate::{
    generate_rust_bindings, rust_pascal_identifier, rust_snake_identifier, unit_type,
};
use super::{RustBindingGenerationError as Error, RustBindingModule, RustBindingOptions};
use crate::CheckedNativeType;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
#[path = "generate_prepared_storage/emit.rs"]
mod emit;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedStorageGenerationError {
    Binding(Error),
    Unsupported(String),
    Missing(String),
    Bound,
}
impl From<Error> for PreparedStorageGenerationError {
    fn from(e: Error) -> Self {
        Self::Binding(e)
    }
}
type R<T> = Result<T, PreparedStorageGenerationError>;
pub(super) enum SlotKind {
    Text(usize),
    Primitive(String),
    Variant(Vec<(String, String)>),
}
pub(super) struct Slot {
    path: String,
    kind: SlotKind,
    rust: String,
}
pub(super) struct Layout {
    slots: Vec<Slot>,
    dependencies: BTreeSet<String>,
    expression: String,
}
/// Appends only root storage implementations and validates their complete closed
/// dependency set. Unsupported shapes refuse; no existing binding is rewritten.
pub fn generate_rust_bindings_with_prepared_storage(
    types: &[CheckedNativeType],
    options: &RustBindingOptions,
    root_names: &[&str],
) -> R<RustBindingModule> {
    let mut module = generate_rust_bindings(types, options)?;
    let mut roots = BTreeSet::new();
    for root in root_names {
        if !roots.insert(*root) {
            return Err(PreparedStorageGenerationError::Unsupported((*root).into()));
        }
        let native = types
            .iter()
            .find(|n| n.name == *root)
            .ok_or_else(|| PreparedStorageGenerationError::Missing((*root).into()))?;
        let mut layout = Layout {
            slots: Vec::new(),
            dependencies: BTreeSet::new(),
            expression: String::new(),
        };
        let prefix = if options.type_prefix.is_empty() {
            String::new()
        } else {
            rust_pascal_identifier(&options.type_prefix)?
        };
        let names: BTreeMap<_, _> = types
            .iter()
            .map(|n| {
                Ok((
                    n.identity.as_str().to_string(),
                    format!("{prefix}{}", rust_pascal_identifier(&n.name)?),
                ))
            })
            .collect::<Result<_, Error>>()?;
        layout.expression = expression(
            &native.value_type,
            "value".into(),
            None,
            types,
            &names,
            &mut layout,
            0,
        )?;
        if layout.slots.is_empty() || layout.slots.len() > 4096 {
            return Err(PreparedStorageGenerationError::Bound);
        }
        let rust = &names[native.identity.as_str()];
        emit::emit(&mut module.source, native, rust, &layout, types, &names)?;
    }
    Ok(module)
}
fn expression(
    ty: &StructuredInfoType,
    path: String,
    bound: Option<usize>,
    types: &[CheckedNativeType],
    names: &BTreeMap<String, String>,
    layout: &mut Layout,
    depth: usize,
) -> R<String> {
    if depth > 64 || layout.slots.len() > 4096 {
        return Err(PreparedStorageGenerationError::Bound);
    }
    match ty.shape() {
        StructuredInfoTypeShape::Nominal {
            schema,
            representation,
        } => {
            let n = dependency(schema.as_str(), ty, types, layout)?;
            let bound = n
                .value_contracts
                .iter()
                .find(|c| c.representation_path.is_empty())
                .map(|c| c.contract.maximum_bytes as usize)
                .or(bound);
            Ok(format!(
                "{}({})",
                names[schema.as_str()],
                expression(representation, path, bound, types, names, layout, depth + 1)?
            ))
        }
        StructuredInfoTypeShape::Record { schema, fields } => {
            dependency(schema.as_str(), ty, types, layout)?;
            let fields=fields.iter().map(|f|Ok(format!("{}: {}",rust_snake_identifier(f.name())?,expression(f.value_type(),format!("{path}.representation().map_err(|_| E::Canonical)?.record_field({:?}).map_err(|_| E::Canonical)?.ok_or(E::Canonical)?",f.name()),None,types,names,layout,depth+1)?))).collect::<R<Vec<_>>>()?;
            Ok(format!(
                "{} {{ {} }}",
                names[schema.as_str()],
                fields.join(",")
            ))
        }
        StructuredInfoTypeShape::Collection { element, length } => {
            if !matches!(element.shape(), StructuredInfoTypeShape::Leaf(_)) || *length > 256 {
                return Err(PreparedStorageGenerationError::Unsupported(
                    "nonprimitive/large fixed array".into(),
                ));
            }
            let values=(0..*length).map(|i|expression(element,format!("{path}.representation().map_err(|_| E::Canonical)?.collection_index({i}).map_err(|_| E::Canonical)?.ok_or(E::Canonical)?"),bound,types,names,layout,depth+1)).collect::<R<Vec<_>>>()?;
            Ok(format!("[{}]", values.join(",")))
        }
        StructuredInfoTypeShape::Leaf(kind) => {
            let (slot, rust) = if kind.as_str() == "value/text" {
                let max = bound.filter(|n| *n <= 65536).ok_or_else(|| {
                    PreparedStorageGenerationError::Unsupported("unbounded text".into())
                })?;
                (SlotKind::Text(max), "String".into())
            } else {
                let rust = match kind.as_str() {
                    "value/u8" => "u8",
                    "value/u16" => "u16",
                    "value/u32" => "u32",
                    "value/u64" => "u64",
                    "value/u128" => "u128",
                    "value/i8" => "i8",
                    "value/i16" => "i16",
                    "value/i32" => "i32",
                    "value/i64" => "i64",
                    "value/i128" => "i128",
                    "value/bool" => "bool",
                    _ => {
                        return Err(PreparedStorageGenerationError::Unsupported(
                            kind.as_str().into(),
                        ))
                    }
                };
                (SlotKind::Primitive(kind.as_str().into()), rust.into())
            };
            let index = layout.slots.len();
            let text = matches!(slot, SlotKind::Text(_));
            layout.slots.push(Slot {
                path,
                kind: slot,
                rust,
            });
            Ok(if text {
                format!("self.f{index}")
            } else {
                format!("self.f{index}.ok_or(E::State)?")
            })
        }
        StructuredInfoTypeShape::Variant { schema, cases } => {
            dependency(schema.as_str(), ty, types, layout)?;
            if !cases.iter().all(|c| unit_type(c.payload_type())) || cases.is_empty() {
                return Err(PreparedStorageGenerationError::Unsupported(
                    "payload/empty variant".into(),
                ));
            }
            let rust = names[schema.as_str()].clone();
            let cases = cases
                .iter()
                .map(|c| Ok((c.tag().to_string(), rust_pascal_identifier(c.tag())?)))
                .collect::<Result<Vec<_>, Error>>()?;
            let index = layout.slots.len();
            layout.slots.push(Slot {
                path,
                kind: SlotKind::Variant(cases),
                rust,
            });
            Ok(format!("self.f{index}.ok_or(E::State)?"))
        }
        _ => Err(PreparedStorageGenerationError::Unsupported(
            "sequence/optional/external shape".into(),
        )),
    }
}
fn dependency<'a>(
    id: &str,
    ty: &StructuredInfoType,
    types: &'a [CheckedNativeType],
    layout: &mut Layout,
) -> R<&'a CheckedNativeType> {
    let n = types
        .iter()
        .find(|n| n.identity.as_str() == id)
        .ok_or_else(|| PreparedStorageGenerationError::Missing(id.into()))?;
    if &n.value_type != ty {
        return Err(PreparedStorageGenerationError::Unsupported(
            "conflicting nested schema".into(),
        ));
    }
    layout.dependencies.insert(id.into());
    Ok(n)
}
