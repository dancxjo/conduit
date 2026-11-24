//! Hosted sink factory for the canonical resident Patchbay Fore.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::scheduler::HostCallSinkBack;

pub(super) use conduit_std_offers::{
    PATCHBAY_INTERACTION_IMPLEMENTATION as IMPLEMENTATION,
    PATCHBAY_INTERACTION_OPERATION as OPERATION,
};
const BYTES: u32 = conduit_face::MAX_FACE_INTERACTION_BYTES as u32;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let expected = conduit_std_offers::patchbay_interaction_offer();
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.semantic_contract != expected.semantic_contract
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.host_calls != expected.host_calls
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("Patchbay interaction placement differs from its exact hosted Back".into());
    }
    // Fore ingress owns the input bytes. The sink only borrows that value.
    // The owner route separately funds the finite interaction history.
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 0,
        maximum_value_bytes: BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(placement)?;
    Ok(InstalledBack::PatchbayInteraction(HostCallSinkBack::new(
        BYTES,
    )))
}
