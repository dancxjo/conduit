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
        Self::admission_metadata_for(core::slice::from_ref(self))
    }
    pub(crate) fn admission_metadata_for(
        witnesses: &[NativeLawWitness],
    ) -> Result<Vec<CheckedNativeType>, NativeLawCustodyRefusal> {
        if witnesses.len() > MAXIMUM_TYPES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        let mut source_budget = Budget::default();
        for witness in witnesses {
            admit_witness(witness, 0, &mut source_budget)?;
        }
        let mut batches = Vec::new();
        let mut metadata_budget = Budget::default();
        let mut count = 0;
        for witness in witnesses {
            replay(witness, &mut batches, &mut metadata_budget, &mut count)?;
        }
        // Preserve the first exact requested root spelling, as the single-owner
        // facade always did, while merging equivalent schema aliases below.
        let mut retained = Vec::new();
        if let Some(first) = witnesses.first() {
            let (batch, root) = batches
                .iter()
                .enumerate()
                .find_map(|(index, batch)| {
                    batch
                        .iter()
                        .position(|ty| ty == first.checked_type())
                        .map(|root| (index, root))
                })
                .ok_or(NativeLawCustodyRefusal::Type)?;
            retained.push(batches[batch].swap_remove(root));
        }
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
        for witness in witnesses {
            if !retained.iter().any(|ty| witness.agrees(ty)) {
                return Err(NativeLawCustodyRefusal::Type);
            }
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
