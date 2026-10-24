//! Recover checked nested owners from opaque Source custody for retained admission.
use super::*;

const MAXIMUM_TYPES: usize = 4096;
impl NativeLawWitness {
    /// No public metadata list can enter this factory. The complete opaque graph
    /// is bounded before checker replay; checker-produced metadata is separately
    /// bounded before it is retained or cloned into an admission engine.
    pub(crate) fn admission_metadata(
        &self,
    ) -> Result<Vec<CheckedNativeType>, NativeLawCustodyRefusal> {
        admit_witness(self, 0, &mut Budget::default())?;
        let mut batches = Vec::new();
        replay(self, &mut batches, &mut Budget::default(), &mut 0)?;
        // Keep the exact requested root spelling when equivalent import aliases
        // share a schema; conflict checks still compare all law metadata.
        let root_batch = batches.last_mut().ok_or(NativeLawCustodyRefusal::Type)?;
        let root = root_batch
            .iter()
            .position(|ty| ty == self.checked_type())
            .ok_or(NativeLawCustodyRefusal::Type)?;
        let mut retained = alloc::vec![root_batch.swap_remove(root)];
        for ty in batches.into_iter().flatten() {
            if let Some(prior) = retained.iter().find(|prior| prior.identity == ty.identity) {
                if prior.value_type != ty.value_type
                    || prior.value_contracts != ty.value_contracts
                    || prior.invariants != ty.invariants
                {
                    return Err(NativeLawCustodyRefusal::Laws);
                }
            } else {
                retained.push(ty);
            }
        }
        if !retained.iter().any(|ty| self.agrees(ty)) {
            return Err(NativeLawCustodyRefusal::Type);
        }
        Ok(retained)
    }
}
fn replay(
    witness: &NativeLawWitness,
    batches: &mut Vec<Vec<CheckedNativeType>>,
    budget: &mut Budget,
    count: &mut usize,
) -> Result<(), NativeLawCustodyRefusal> {
    let mut base = StartupCatalog::new();
    for (name, child) in &witness.module.imports {
        replay(child, batches, budget, count)?;
        base.insert_checked_native_type(name.clone(), child.checked_type())
            .map_err(|_| NativeLawCustodyRefusal::DuplicateImport)?;
    }
    for (name, family) in &witness.module.families {
        family.install(name, &mut base)?;
    }
    let checked = check_module(&witness.module.source, &base)?;
    if !checked.iter().any(|ty| ty == witness.checked_type()) {
        return Err(NativeLawCustodyRefusal::Laws);
    }
    *count = count
        .checked_add(checked.len())
        .ok_or(NativeLawCustodyRefusal::Capacity)?;
    if *count > MAXIMUM_TYPES {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    for ty in &checked {
        budget.claims(ty)?;
    }
    // Existing bounded checking creates its result metadata; retaining it moves
    // those values only after the aggregate admission, without cloning them.
    batches.push(checked);
    Ok(())
}
