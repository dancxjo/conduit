//! Private metadata origins. Inspection vectors are never a new law authority.
use super::*;

pub(super) enum Metadata<'d> {
    Borrowed(&'d CheckedSyntaxDocument),
    Owned(Vec<CheckedNativeType>),
    Shared(registry::SharedRegistry),
}
impl Metadata<'_> {
    pub(super) fn get(&self, index: usize) -> &CheckedNativeType {
        match self {
            Self::Borrowed(document) => &document.retained_native_types[index],
            Self::Owned(types) => &types[index],
            Self::Shared(registry) => &registry.types[index],
        }
    }
    // Only the new sealed registry profile checks arithmetic regardless of the
    // original proven hints. Legacy borrowed/owned profiles stay unchanged.
    pub(super) fn checked_arithmetic(&self) -> bool {
        matches!(self, Self::Shared(_))
    }
    pub(super) fn permits_structural(&self, ty: &StructuredInfoType) -> bool {
        matches!(self, Self::Shared(_)) && structural_root::exact_constructor(ty)
    }
    pub(super) fn lookup(&self, identity: &KindId) -> Result<usize, PreparedNativeRefusal> {
        let types = match self {
            Self::Borrowed(document) => {
                traversal::lookup(document, identity)?;
                &document.retained_native_types
            }
            Self::Owned(types) => types,
            Self::Shared(registry) => &registry.types,
        };
        let index = types
            .iter()
            .position(|ty| &ty.identity == identity)
            .ok_or(PreparedNativeRefusal::MissingNativeMetadata)?;
        let owner = &types[index];
        for other in types.iter().filter(|ty| &ty.identity == identity) {
            if other.value_type != owner.value_type
                || other.value_contracts != owner.value_contracts
                || other.invariants != owner.invariants
            {
                return Err(PreparedNativeRefusal::ConflictingNativeMetadata);
            }
        }
        Ok(index)
    }
}
