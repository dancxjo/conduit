//! Exact constructor-derived roots carry structural proof, never Native fiction.
use super::*;

pub(super) enum RootDescriptor {
    Native(usize),
    Structural(StructuredInfoType),
}
impl RootDescriptor {
    pub(super) fn value_type<'a>(
        &'a self,
        metadata: &'a metadata::Metadata<'_>,
    ) -> &'a StructuredInfoType {
        match self {
            Self::Native(index) => &metadata.get(*index).value_type,
            Self::Structural(ty) => ty,
        }
    }
}
pub(super) fn descriptor(
    metadata: &metadata::Metadata<'_>,
    ty: &StructuredInfoType,
) -> Result<RootDescriptor, PreparedNativeRefusal> {
    coverage(metadata, ty, 0, &mut 0)?;
    if let Some(identity) = identity(ty) {
        if let Ok(index) = metadata.lookup(identity) {
            return Ok(RootDescriptor::Native(index));
        }
    }
    Ok(RootDescriptor::Structural(ty.clone()))
}
fn identity(ty: &StructuredInfoType) -> Option<&KindId> {
    match ty.shape() {
        Shape::Nominal { schema, .. }
        | Shape::Record { schema, .. }
        | Shape::Variant { schema, .. } => Some(schema),
        _ => None,
    }
}
pub(super) fn exact_constructor(ty: &StructuredInfoType) -> bool {
    if traversal::exact_core_optional(ty) {
        return true;
    }
    let Shape::Record { fields, .. } = ty.shape() else {
        return false;
    };
    crate::expression_check::anonymous_record("record", fields.to_vec())
        .is_ok_and(|expected| &expected == ty)
        || conduit_core::tuple_info_type(
            fields
                .iter()
                .map(|field| field.value_type().clone())
                .collect(),
        )
        .is_ok_and(|expected| &expected == ty)
}
fn coverage(
    metadata: &metadata::Metadata<'_>,
    ty: &StructuredInfoType,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), PreparedNativeRefusal> {
    *nodes = nodes.checked_add(1).ok_or(PreparedNativeRefusal::Bounds)?;
    if depth > 64 || *nodes > 4096 {
        return Err(PreparedNativeRefusal::Bounds);
    }
    if let Some(identity) = identity(ty) {
        match metadata.lookup(identity) {
            Ok(index) if &metadata.get(index).value_type == ty => (),
            Ok(_) => return Err(PreparedNativeRefusal::ConflictingNativeMetadata),
            Err(PreparedNativeRefusal::MissingNativeMetadata) if exact_constructor(ty) => (),
            Err(reason) => return Err(reason),
        }
    }
    match ty.shape() {
        Shape::Leaf(kind) => {
            if conduit_core::primitive_info_kind(kind.as_str()).is_none()
                && kind.as_str() != conduit_core::RESOURCE_REFERENCE_INFO_ID
            {
                return Err(PreparedNativeRefusal::MissingNativeMetadata);
            }
        }
        Shape::Nominal { representation, .. } => {
            coverage(metadata, representation, depth + 1, nodes)?
        }
        Shape::Record { fields, .. } => {
            for field in fields {
                coverage(metadata, field.value_type(), depth + 1, nodes)?;
            }
        }
        Shape::Variant { cases, .. } => {
            for case in cases {
                coverage(metadata, case.payload_type(), depth + 1, nodes)?;
            }
        }
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            coverage(metadata, element, depth + 1, nodes)?
        }
    }
    Ok(())
}
