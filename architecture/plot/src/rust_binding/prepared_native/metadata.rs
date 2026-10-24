//! Private metadata origins. Inspection vectors are never a new law authority.
use super::*;

pub(super) enum Metadata<'d> {
    Borrowed(&'d CheckedSyntaxDocument),
    Owned(Vec<CheckedNativeType>),
}
impl Metadata<'_> {
    pub(super) fn get(&self, index: usize) -> &CheckedNativeType {
        match self {
            Self::Borrowed(document) => &document.retained_native_types[index],
            Self::Owned(types) => &types[index],
        }
    }
    pub(super) fn lookup(&self, identity: &KindId) -> Result<usize, PreparedNativeRefusal> {
        let types = match self {
            Self::Borrowed(document) => {
                traversal::lookup(document, identity)?;
                &document.retained_native_types
            }
            Self::Owned(types) => types,
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
