//! Expansion Kind read congruence, with whole physical/Type invariant context.
#[cfg(target_has_atomic = "ptr")]
use crate::preparation_cache::read_trace::{CatalogueRead, CatalogueTable, TraceHandle};
use crate::ProfileCatalog;
use conduit_core::KindId;
impl ProfileCatalog {
    #[inline]
    pub(crate) fn observe_projection(&self, key: &KindId) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace
            .record(CatalogueTable::Projection, key.as_str());
        #[cfg(not(target_has_atomic = "ptr"))]
        let _ = key;
    }
    #[inline]
    pub(crate) fn observe_canonical(&self, key: &KindId) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace
            .record(CatalogueTable::CanonicalKind, key.as_str());
        #[cfg(not(target_has_atomic = "ptr"))]
        let _ = key;
    }
    #[inline]
    pub(crate) fn observe_variadic_fore(&self, key: &KindId) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace
            .record(CatalogueTable::VariadicFore, key.as_str());
        #[cfg(not(target_has_atomic = "ptr"))]
        let _ = key;
    }
    #[inline]
    pub(crate) fn observe_variadic_kind(&self, key: &KindId) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace
            .record(CatalogueTable::VariadicKind, key.as_str());
        #[cfg(not(target_has_atomic = "ptr"))]
        let _ = key;
    }
    pub(crate) fn observe_all_kinds(&self) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace.observe_all();
    }
    #[cfg(target_has_atomic = "ptr")]
    pub(crate) fn traced(&self, trace: TraceHandle) -> Self {
        let mut observed = self.clone();
        observed.read_trace = trace;
        observed
    }
    #[cfg(target_has_atomic = "ptr")]
    pub(crate) fn same_read_context(&self, other: &Self, reads: &[CatalogueRead]) -> bool {
        let Self {
            physical,
            kinds,
            canonical_kinds,
            variadic_fores,
            variadic_kinds,
            type_invariants,
            read_trace: _,
        } = self;
        let Self {
            physical: rhs_physical,
            kinds: rhs_kinds,
            canonical_kinds: rhs_canonical,
            variadic_fores: rhs_fores,
            variadic_kinds: rhs_variadic,
            type_invariants: rhs_invariants,
            read_trace: _,
        } = other;
        physical == rhs_physical
            && type_invariants == rhs_invariants
            && reads.iter().all(|read| {
                let key = KindId::from(read.key.as_str());
                match read.table {
                    CatalogueTable::Projection => kinds.get(&key) == rhs_kinds.get(&key),
                    CatalogueTable::CanonicalKind => {
                        canonical_kinds.get(&key) == rhs_canonical.get(&key)
                    }
                    CatalogueTable::VariadicFore => variadic_fores.get(&key) == rhs_fores.get(&key),
                    CatalogueTable::VariadicKind => {
                        variadic_kinds.get(&key) == rhs_variadic.get(&key)
                    }
                    _ => false,
                }
            })
    }
}
