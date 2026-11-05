//! One authoritative lookup for checked Native owners, independent of bindings.
use super::*;
#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeOwnerRefusal {
    Missing,
    Conflicting,
}
impl CheckedSyntaxDocument {
    pub(crate) fn retained_native_owner(
        &self,
        identity: &conduit_core::KindId,
    ) -> Result<&CheckedNativeType, NativeOwnerRefusal> {
        let owner = self
            .retained_native_types
            .iter()
            .find(|native| &native.identity == identity)
            .ok_or(NativeOwnerRefusal::Missing)?;
        for native in self
            .retained_native_types
            .iter()
            .chain(self.native_types.iter())
            .filter(|native| &native.identity == identity)
        {
            if owner.value_type != native.value_type
                || owner.value_contracts != native.value_contracts
                || owner.invariants != native.invariants
            {
                return Err(NativeOwnerRefusal::Conflicting);
            }
        }
        Ok(owner)
    }
}
