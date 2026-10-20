//! Checking Kind/Fore read congruence; all other contexts remain exact.
use super::StartupCatalog;
#[cfg(target_has_atomic = "ptr")]
use crate::preparation_cache::read_trace::{CatalogueRead, CatalogueTable, TraceHandle};

impl StartupCatalog {
    #[inline]
    pub(super) fn observe_signature(&self, key: &str) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace.record(CatalogueTable::Signature, key);
        #[cfg(not(target_has_atomic = "ptr"))]
        let _ = key;
    }
    #[inline]
    pub(super) fn observe_fore(&self, key: &str) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace.record(CatalogueTable::Fore, key);
        #[cfg(not(target_has_atomic = "ptr"))]
        let _ = key;
    }
    #[inline]
    pub(super) fn observe_variadic(&self, key: &str) {
        #[cfg(target_has_atomic = "ptr")]
        self.read_trace.record(CatalogueTable::VariadicFore, key);
        #[cfg(not(target_has_atomic = "ptr"))]
        let _ = key;
    }
    #[cfg(target_has_atomic = "ptr")]
    pub(crate) fn traced(&self, trace: TraceHandle) -> Self {
        let mut observed = self.clone();
        observed.read_trace = trace;
        observed
    }
    #[cfg(target_has_atomic = "ptr")]
    pub(crate) fn same_read_context(&self, other: &Self, reads: &[CatalogueRead]) -> bool {
        // Exhaustive patterns deliberately fail compilation when a new context
        // field is added: it cannot silently escape invalidation.
        let Self {
            read_trace: _,
            physical,
            physical_sources,
            literal_owners,
            kinds,
            structured_defaults,
            prepared_source_values,
            prepared_glyph_values,
            typed_literal_families,
            fores,
            variadic_fores,
            structured_types,
            structured_type_contracts,
            structured_type_invariants,
            value_kind_aliases,
            native_type_sources,
            native_families,
            native_refinement_diagnostics,
            exact_initial_info,
        } = self;
        let Self {
            read_trace: _,
            physical: rhs_physical,
            physical_sources: rhs_physical_sources,
            literal_owners: rhs_literal_owners,
            kinds: rhs_kinds,
            structured_defaults: rhs_structured_defaults,
            prepared_source_values: rhs_prepared_source_values,
            prepared_glyph_values: rhs_prepared_glyph_values,
            typed_literal_families: rhs_typed_literal_families,
            fores: rhs_fores,
            variadic_fores: rhs_variadic_fores,
            structured_types: rhs_structured_types,
            structured_type_contracts: rhs_structured_type_contracts,
            structured_type_invariants: rhs_structured_type_invariants,
            value_kind_aliases: rhs_value_kind_aliases,
            native_type_sources: rhs_native_type_sources,
            native_families: rhs_native_families,
            native_refinement_diagnostics: rhs_native_refinement_diagnostics,
            exact_initial_info: rhs_exact_initial_info,
        } = other;
        physical == rhs_physical
            && physical_sources == rhs_physical_sources
            && literal_owners == rhs_literal_owners
            && structured_defaults == rhs_structured_defaults
            && prepared_source_values == rhs_prepared_source_values
            && prepared_glyph_values == rhs_prepared_glyph_values
            && typed_literal_families == rhs_typed_literal_families
            && structured_types == rhs_structured_types
            && structured_type_contracts == rhs_structured_type_contracts
            && structured_type_invariants == rhs_structured_type_invariants
            && value_kind_aliases == rhs_value_kind_aliases
            && native_type_sources == rhs_native_type_sources
            && native_families == rhs_native_families
            && native_refinement_diagnostics == rhs_native_refinement_diagnostics
            && exact_initial_info == rhs_exact_initial_info
            && reads.iter().all(|read| match read.table {
                CatalogueTable::Signature => kinds.get(&read.key) == rhs_kinds.get(&read.key),
                CatalogueTable::Fore => fores.get(&read.key) == rhs_fores.get(&read.key),
                CatalogueTable::VariadicFore => {
                    variadic_fores.get(&read.key) == rhs_variadic_fores.get(&read.key)
                }
                _ => false,
            })
    }
}
