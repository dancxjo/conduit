//! Reuse the original materializer/verifier preparation receipts.
//! These descriptors model storage only; ordinary verification still executes.
use super::full_refusal::FullPreparationRefusal as Refusal;
use conduit_core::Plan;
use conduit_plan_lowering::lowering::fragment_lowering_layout;

pub(super) fn preparation_bytes(plan: &Plan) -> Result<usize, Refusal> {
    if plan.fragments.len() != 1 {
        return Err(Refusal::Unsupported);
    }
    let layout = fragment_lowering_layout(&plan.fragments[0]).map_err(|error| match error {
        conduit_plan_lowering::lowering::LoweringError::CapacityOverflow => Refusal::Pressure,
        _ => Refusal::Unsupported,
    })?;
    if layout.nodes != 1 || layout.cords != 3 || layout.fore_ports != 3 {
        return Err(Refusal::Unsupported);
    }
    let verification =
        conduit_core::local_plan_verification_backing_bytes(plan).ok_or(Refusal::Unsupported)?;
    layout
        .retained_owned_bytes
        .checked_add(layout.refusal_owned_bytes)
        .and_then(|n| n.checked_add(verification))
        .ok_or(Refusal::Pressure)
}
