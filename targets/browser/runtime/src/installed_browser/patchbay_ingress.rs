//! Explicit preparation primitive for the canonical Patchbay sink.
//! Not advertised by the legacy browser profile until its owner route is wired.
use conduit_core::{Back, BackOfferBuilder, CapabilityOffer, HostCallRequirement, PlannedGear};
use conduit_kernel::scheduler::StepBack;

pub const OPERATION: &str = "conduit.host/browser-patchbay-accept-interaction@1";
const IMPLEMENTATION: &str = "browser/patchbay-accept-interaction@1";
const BYTES: u32 = conduit_face::MAX_FACE_INTERACTION_BYTES as u32;

/// This offer names no ambient owner. An installing owner must bind the exact
/// resident instance, retain current Face/Show evidence and reserve its typed
/// input cord and Host Call storage before admitting this Back to Play.
pub fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        patchbay_application::patchbay_interaction_contract(),
        Back {
            capability_id: IMPLEMENTATION.into(),
            execution_profile_id: "browser/bounded-patchbay-interaction@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: "conduit-browser-runtime/patchbay-interaction@1".into(),
            host_calls: vec![HostCallRequirement {
                contract_id: OPERATION.into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: BYTES,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

/// Reuse the installed finite Host Call sink verb: one pending call, no initial
/// sentinel, no output and completion only after input closes. Owner dispatch
/// must run `apply_face_interaction` against independently retained evidence.
pub fn prepare(
    placement: &PlannedGear,
) -> Result<impl StepBack<{ crate::installed_browser::BROWSER_PORTS_PER_GEAR }>, String> {
    let expected = offer();
    super::factory::validate_placement(placement, &expected)?;
    if placement.capability_id != expected.capability_id
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned Patchbay ingress differs from its exact finite offer".into());
    }
    Ok(crate::installed_browser::BrowserBack::face(BYTES, 1))
}

#[cfg(test)]
#[path = "patchbay_ingress_tests.rs"]
mod tests;

#[path = "patchbay_ingress_plan.rs"]
mod plan;
pub use plan::PlannedPatchbayIngress;
