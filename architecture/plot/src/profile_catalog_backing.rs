//! Retention-only accounting for the compiler-owned profile catalogue.
use crate::{
    preparation_backing::{sum, Backing},
    KindProjection, ProfileCatalog,
};
impl Backing for KindProjection {
    fn backing(&self) -> Option<usize> {
        sum([
            self.kind_id.backing(),
            self.kind_contract_revision.backing(),
            self.inputs.backing(),
            self.outputs.backing(),
            self.configuration.backing(),
        ])
    }
}
impl ProfileCatalog {
    /// Conservative reachable heap storage, excluding inline Self/allocator
    /// overhead. Shared payloads are charged independently per reachable alias;
    /// this is neither exclusive backing nor a synchronous factory peak.
    /// Unsupported descendants and arithmetic overflow return None.
    pub fn owned_preparation_backing_bytes(&self) -> Option<usize> {
        #[cfg(target_has_atomic = "ptr")]
        let trace = self.read_trace.owned_preparation_backing_bytes();
        #[cfg(not(target_has_atomic = "ptr"))]
        let trace = Some(0);
        sum([
            trace,
            self.physical.owned_preparation_backing_bytes(),
            self.kinds.backing(),
            self.canonical_kinds.backing(),
            self.variadic_fores.backing(),
            self.variadic_kinds.backing(),
            self.type_invariants.backing(),
        ])
    }
}
