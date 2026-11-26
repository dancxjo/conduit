//! Reuse the original materializer/verifier preparation receipts.
//! These descriptors model storage only; ordinary verification still executes.
use conduit_core::Plan;
use conduit_plan_lowering::lowering::fragment_lowering_layout;

pub(super) fn preparation_bytes(plan: &Plan) -> Result<usize, String> {
    if plan.fragments.len() != 1 {
        return Err("Full Mask requires one generic fragment".into());
    }
    let layout = fragment_lowering_layout(&plan.fragments[0])
        .map_err(|e| format!("Full Mask lowering storage: {e:?}"))?;
    if layout.nodes != 1 || layout.cords != 3 || layout.fore_ports != 3 {
        return Err("Full Mask generic lowering shape".into());
    }
    let verification = conduit_core::local_plan_verification_backing_bytes(plan)
        .ok_or("Full Mask verification storage unsupported")?;
    layout
        .retained_owned_bytes
        .checked_add(layout.refusal_owned_bytes)
        .and_then(|n| n.checked_add(verification))
        .ok_or_else(|| "Full Mask lowering storage overflow".into())
}
