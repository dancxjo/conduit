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
#[path = "generate_prepared_storage/variant.rs"]
mod variant;
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
    Tag(Vec<String>),
}
pub(super) struct Slot {
    path: String,
    kind: SlotKind,
    rust: String,
    guard: String,
}
pub(super) struct Layout {
    slots: Vec<Slot>,
    dependencies: BTreeSet<String>,
    expression: String,
    branched: bool,
    boxed: BTreeSet<String>,
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
            branched: false,
            boxed: options.boxed_variant_payloads.iter().cloned().collect(),
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
            "",
        )?;
        for (index, slot) in layout.slots.iter().enumerate() {
            if matches!(slot.kind, SlotKind::Text(_)) {
                layout.expression = layout.expression.replace(
                    &format!("__prepared_text_{index}__"),
                    &if layout.branched {
                        format!("self.f{index}.take().ok_or(E::State)?")
                    } else {
                        format!("self.f{index}")
                    },
                );
            }
        }
        if layout.slots.is_empty() || layout.slots.len() > 4096 {
            return Err(PreparedStorageGenerationError::Bound);
        }
        let rust = &names[native.identity.as_str()];
        emit::emit(&mut module.source, native, rust, &layout, types, &names)?;
    }
    Ok(module)
}
// Preparation-only recursive schema traversal; the independent branch guard must
// travel with the canonical path and exact dependency tables. No Play API grows.
#[allow(clippy::too_many_arguments)]
fn expression(
    ty: &StructuredInfoType,
    path: String,
    bound: Option<usize>,
    types: &[CheckedNativeType],
    names: &BTreeMap<String, String>,
    layout: &mut Layout,
    depth: usize,
    guard: &str,
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
                expression(
                    representation,
                    path,
                    bound,
                    types,
                    names,
                    layout,
                    depth + 1,
                    guard
                )?
            ))
        }
        StructuredInfoTypeShape::Record { schema, fields } => {
            dependency(schema.as_str(), ty, types, layout)?;
            let fields=fields.iter().map(|f|Ok(format!("{}: {}",rust_snake_identifier(f.name())?,expression(f.value_type(),format!("{path}.representation().map_err(|_| E::Canonical)?.record_field({:?}).map_err(|_| E::Canonical)?.ok_or(E::Canonical)?",f.name()),None,types,names,layout,depth+1,guard)?))).collect::<R<Vec<_>>>()?;
            Ok(format!(
                "{} {{ {} }}",
                names[schema.as_str()],
                fields.join(",")
            ))
        }
        StructuredInfoTypeShape::Collection { element, length } => {
            if !matches!(element.shape(), StructuredInfoTypeShape::Leaf(_)) || length > 256 {
                return Err(PreparedStorageGenerationError::Unsupported(
                    "nonprimitive/large fixed array".into(),
                ));
            }
            let values=(0..length).map(|i|expression(element,format!("{path}.representation().map_err(|_| E::Canonical)?.collection_index({i}).map_err(|_| E::Canonical)?.ok_or(E::Canonical)?"),bound,types,names,layout,depth+1,guard)).collect::<R<Vec<_>>>()?;
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
                guard: guard.into(),
            });
            Ok(if text {
                format!("__prepared_text_{index}__")
            } else {
                format!("self.f{index}.ok_or(E::State)?")
            })
        }
        StructuredInfoTypeShape::Variant { schema, cases } => {
            if schema.as_str() == "conduit.conduitese.optional.v1"
                || !cases.iter().all(|c| unit_type(c.payload_type()))
            {
                return variant::expression(ty, path, types, names, layout, depth, guard);
            }
            dependency(schema.as_str(), ty, types, layout)?;
            if cases.is_empty() {
                return Err(PreparedStorageGenerationError::Unsupported(
                    "empty variant".into(),
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
                guard: guard.into(),
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
