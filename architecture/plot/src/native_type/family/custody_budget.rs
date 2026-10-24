//! Optional preparation metadata has its own finite registry profile. It never
//! consumes the legacy Source family dependency budget or changes admission.
use super::NativeTypeFamily;
use crate::StartupCatalog;

const MAXIMUM_MODULES: usize = 256;
const MAXIMUM_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct Budget {
    modules: usize,
    bytes: usize,
}
impl Budget {
    fn admit(&mut self, bounds: Option<(usize, usize)>) -> bool {
        let Some((modules, bytes)) = bounds else {
            return false;
        };
        let modules = self.modules.saturating_add(modules);
        let bytes = self.bytes.saturating_add(bytes);
        if modules > MAXIMUM_MODULES || bytes > MAXIMUM_BYTES {
            return false;
        }
        self.modules = modules;
        self.bytes = bytes;
        true
    }
    fn retain(&mut self, family: &mut NativeTypeFamily) {
        if family
            .source_custody
            .as_ref()
            .is_some_and(|witness| !self.admit(witness.bounds().ok()))
        {
            family.source_custody = None;
        }
        family.dependency_custody.retain(|name, witness| {
            family.dependencies.iter().any(|ty| {
                ty.name == *name
                    && witness.checked_type().value_type == ty.value_type
                    && witness.checked_type().value_contracts == ty.value_contracts
                    && witness.checked_type().invariants == ty.invariants
            }) && self.admit(witness.preparation_bounds().ok())
        });
    }
}

/// Keep source custody first, then concrete dependency custody in name order.
/// A dropped optional witness is explicitly absent, never an empty law claim.
pub(crate) fn retain_optional(catalog: &StartupCatalog, candidate: &mut NativeTypeFamily) {
    let mut budget = Budget::default();
    for family in catalog.native_families.values() {
        if let Some(witness) = &family.source_custody {
            if !budget.admit(witness.bounds().ok()) {
                candidate.source_custody = None;
                candidate.dependency_custody.clear();
                return;
            }
        }
        for witness in family.dependency_custody.values() {
            if !budget.admit(witness.preparation_bounds().ok()) {
                candidate.source_custody = None;
                candidate.dependency_custody.clear();
                return;
            }
        }
    }
    budget.retain(candidate);
}
