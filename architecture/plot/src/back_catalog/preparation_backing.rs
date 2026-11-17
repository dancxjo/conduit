use super::*;
use crate::preparation_backing::{sum, Backing};
impl CanonicalBackCatalog {
    /// Conservative reachable payload storage, excluding inline Self and
    /// allocator overhead. Shared sources are charged in full per alias; no
    /// exclusivity or factory-peak claim follows from this retained observation.
    pub fn owned_preparation_backing_bytes(&self) -> Option<usize> {
        sum([self.backs.backing(), self.contexts.backing()])
    }
}
impl Backing for CanonicalBackDefinition {
    fn backing(&self) -> Option<usize> {
        sum([self.realization.backing(), self.plot.backing()])
    }
}
impl Backing for CanonicalBackContext {
    fn backing(&self) -> Option<usize> {
        sum([
            self.document.backing(),
            self.catalog.as_ref().map_or(
                Some(0),
                crate::ProfileCatalog::owned_preparation_backing_bytes,
            ),
        ])
    }
}
