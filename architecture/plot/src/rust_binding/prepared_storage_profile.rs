//! Preparation-only physical capacities. Discovery never admits a Native value.
use super::{NativePathStep, PreparedDecodeBounds};
use crate::CheckedNativeType;
use alloc::{format, string::String, vec::Vec};
use conduit_core::{KindId, StructuredInfoType, StructuredInfoTypeShape as Shape};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedStorageLimit {
    SequenceItems(usize),
    TextBytes(usize),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedStorageCapacity {
    pub path: Vec<NativePathStep>,
    pub limit: PreparedStorageLimit,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedStorageProfile {
    pub root: KindId,
    pub capacities: Vec<PreparedStorageCapacity>,
}
/// Owner-declared preparation limits; these do not grant runtime memory.
#[derive(Clone, Copy, Debug)]
pub struct PreparedStorageBounds {
    pub decode: PreparedDecodeBounds,
    pub sequence_items: usize,
    pub storage_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedStorageUnsupported {
    pub path: Vec<NativePathStep>,
    pub reason: String,
}
/// Sites describe semantic maxima. A resolved profile separately records the
/// physical subset; it never changes those maxima or the root Type identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedStorageSchema {
    root: KindId,
    sites: Vec<PreparedStorageCapacity>,
    unsupported: Vec<PreparedStorageUnsupported>,
}
impl PreparedStorageSchema {
    pub fn root(&self) -> &KindId {
        &self.root
    }
    pub fn sites(&self) -> &[PreparedStorageCapacity] {
        &self.sites
    }
    pub fn unsupported(&self) -> &[PreparedStorageUnsupported] {
        &self.unsupported
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedStorageProfileRefusal {
    Bounds,
    MissingRoot,
    ForeignRoot,
    UnsupportedSchema,
    DuplicatePath(Vec<NativePathStep>),
    UnknownPath(Vec<NativePathStep>),
    WrongShape(Vec<NativePathStep>),
    CapacityExceeded(Vec<NativePathStep>),
    MissingSequenceCapacity(Vec<NativePathStep>),
}
type Result<T> = core::result::Result<T, PreparedStorageProfileRefusal>;

/// Resolve each exact typed path before storage allocation. Text defaults to
/// its existing semantic bound; every variable-length sequence needs an
/// explicit physical count, including sequences below an Each or Variant.
pub fn resolve_prepared_storage_profile(
    schema: &PreparedStorageSchema,
    requested: &PreparedStorageProfile,
) -> Result<PreparedStorageProfile> {
    use PreparedStorageLimit::{SequenceItems, TextBytes};
    if schema.root != requested.root {
        return Err(PreparedStorageProfileRefusal::ForeignRoot);
    }
    if !schema.unsupported.is_empty() {
        return Err(PreparedStorageProfileRefusal::UnsupportedSchema);
    }
    if requested.capacities.len() > 4096 || schema.sites.len() > 4096 {
        return Err(PreparedStorageProfileRefusal::Bounds);
    }
    for (index, capacity) in requested.capacities.iter().enumerate() {
        let path = &capacity.path;
        if path.len() > 64 {
            return Err(PreparedStorageProfileRefusal::Bounds);
        }
        if requested.capacities[..index]
            .iter()
            .any(|c| c.path == *path)
        {
            return Err(PreparedStorageProfileRefusal::DuplicatePath(path.clone()));
        }
        let site = schema
            .sites
            .iter()
            .find(|s| s.path == *path)
            .ok_or_else(|| PreparedStorageProfileRefusal::UnknownPath(path.clone()))?;
        let (actual, maximum) = match (&capacity.limit, &site.limit) {
            (SequenceItems(actual), SequenceItems(maximum))
            | (TextBytes(actual), TextBytes(maximum)) => (*actual, *maximum),
            _ => return Err(PreparedStorageProfileRefusal::WrongShape(path.clone())),
        };
        if actual > maximum {
            return Err(PreparedStorageProfileRefusal::CapacityExceeded(
                path.clone(),
            ));
        }
    }
    let mut capacities = Vec::with_capacity(schema.sites.len());
    for site in &schema.sites {
        let selected = requested.capacities.iter().find(|c| c.path == site.path);
        if selected.is_none() && matches!(site.limit, SequenceItems(_)) {
            return Err(PreparedStorageProfileRefusal::MissingSequenceCapacity(
                site.path.clone(),
            ));
        }
        capacities.push(selected.unwrap_or(site).clone());
    }
    Ok(PreparedStorageProfile {
        root: schema.root.clone(),
        capacities,
    })
}

/// Traverse schema templates rather than maximum-count occurrences. This also
/// discovers children of zero-capacity physical sequences. Native laws remain
/// owned by PreparedNativeAdmission and its exact checked witness.
pub fn discover_prepared_storage_schema(
    types: &[CheckedNativeType],
    root: &KindId,
) -> Result<PreparedStorageSchema> {
    let native = types
        .iter()
        .find(|n| n.identity == *root)
        .ok_or(PreparedStorageProfileRefusal::MissingRoot)?;
    let mut discovery = Discovery {
        types,
        schema: PreparedStorageSchema {
            root: root.clone(),
            sites: Vec::new(),
            unsupported: Vec::new(),
        },
        work: 0,
    };
    discovery.walk(&native.value_type, &mut Vec::new(), None)?;
    Ok(discovery.schema)
}
struct Discovery<'a> {
    types: &'a [CheckedNativeType],
    schema: PreparedStorageSchema,
    work: usize,
}
impl Discovery<'_> {
    fn unsupported(&mut self, path: &[NativePathStep], reason: impl Into<String>) {
        self.schema.unsupported.push(PreparedStorageUnsupported {
            path: path.into(),
            reason: reason.into(),
        });
    }
    fn metadata(
        &mut self,
        ty: &StructuredInfoType,
        id: &KindId,
        path: &[NativePathStep],
    ) -> Option<&CheckedNativeType> {
        let native = self.types.iter().find(|n| n.identity == *id);
        match native {
            Some(n) if n.value_type == *ty => Some(n),
            Some(_) => {
                self.unsupported(path, "conflicting Native metadata");
                None
            }
            None => {
                self.unsupported(path, "missing Native metadata");
                None
            }
        }
    }
    fn walk(
        &mut self,
        ty: &StructuredInfoType,
        path: &mut Vec<NativePathStep>,
        bound: Option<usize>,
    ) -> Result<()> {
        use NativePathStep::{Each, Field, Representation, Variant};
        self.work += 1;
        if self.work > 4096 || path.len() > 64 {
            return Err(PreparedStorageProfileRefusal::Bounds);
        }
        match ty.shape() {
            Shape::Nominal {
                schema,
                representation,
            } => {
                let Some(native) = self.metadata(ty, schema, path) else {
                    return Ok(());
                };
                let declared = native
                    .value_contracts
                    .iter()
                    .find(|c| c.representation_path.is_empty())
                    .map(|c| usize::try_from(c.contract.maximum_bytes))
                    .transpose()
                    .map_err(|_| PreparedStorageProfileRefusal::Bounds)?;
                path.push(Representation);
                self.walk(representation, path, declared.or(bound))?;
                path.pop();
            }
            Shape::Record { schema, fields } => {
                if self.metadata(ty, schema, path).is_none() {
                    return Ok(());
                }
                for field in fields {
                    path.push(Field(field.name().into()));
                    self.walk(field.value_type(), path, None)?;
                    path.pop();
                }
            }
            Shape::Sequence {
                element,
                maximum_items,
                ..
            } => {
                self.schema.sites.push(PreparedStorageCapacity {
                    path: path.clone(),
                    limit: PreparedStorageLimit::SequenceItems(maximum_items as usize),
                });
                path.push(Each);
                self.walk(element, path, None)?;
                path.pop();
            }
            Shape::Collection { element, length } => {
                if length > 256 || !matches!(element.shape(), Shape::Leaf(_)) {
                    self.unsupported(path, "nonprimitive/large fixed array");
                } else {
                    path.push(Each);
                    self.walk(element, path, bound)?;
                    path.pop();
                }
            }
            Shape::Variant { schema, cases } => {
                if schema.as_str() == "conduit.conduitese.optional.v1" {
                    let valid = cases
                        .iter()
                        .find(|c| c.tag() == "some")
                        .and_then(|c| {
                            conduit_core::optional_info_type(c.payload_type().clone()).ok()
                        })
                        .is_some_and(|expected| expected == *ty);
                    if !valid {
                        self.unsupported(path, "conflicting core optional schema");
                        return Ok(());
                    }
                } else if self.metadata(ty, schema, path).is_none() {
                    return Ok(());
                }
                if cases.is_empty() || cases.len() > 256 {
                    self.unsupported(path, "empty/large variant");
                } else {
                    for case in cases {
                        if super::generate::unit_type(case.payload_type()) {
                            continue;
                        }
                        path.push(Variant(case.tag().into()));
                        self.walk(case.payload_type(), path, None)?;
                        path.pop();
                    }
                }
            }
            Shape::Leaf(kind) => match kind.as_str() {
                "value/text" => match bound {
                    Some(maximum) if maximum <= 65536 => {
                        self.schema.sites.push(PreparedStorageCapacity {
                            path: path.clone(),
                            limit: PreparedStorageLimit::TextBytes(maximum),
                        })
                    }
                    _ => self.unsupported(path, "unbounded/oversized text"),
                },
                "value/u8" | "value/u16" | "value/u32" | "value/u64" | "value/u128"
                | "value/i8" | "value/i16" | "value/i32" | "value/i64" | "value/i128"
                | "value/bool" => (),
                _ => self.unsupported(path, format!("unsupported primitive {}", kind.as_str())),
            },
        }
        Ok(())
    }
}
