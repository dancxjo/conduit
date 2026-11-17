//! Requested scratch of the original verifier for the reviewed local structural subset.
use crate::{FragmentCommitment, Plan, PlannedConnection};
/// Allocation-free descriptor, not Plan validity or execution authority.
/// Nested dynamic/state/region/Line ownership remains outside this preparation profile.
pub fn local_plan_verification_backing_bytes(plan: &Plan) -> Option<usize> {
    if !plan.activations.is_empty() || !plan.activation_preparations.is_empty() {
        return None;
    }
    let mut bytes = plan
        .source_document_id
        .0
        .len()
        .checked_add(plan.checked_plot_id.0.len())?
        .checked_add(plan.expanded_plot_id.0.len())?;
    bytes = bytes.checked_add(
        plan.fragments
            .len()
            .checked_mul(core::mem::size_of::<FragmentCommitment>())?,
    )?;
    // Every canonical fingerprint creates exactly one owned 32-byte digest's hex String.
    bytes = bytes.checked_add(plan.fragments.len().checked_add(1)?.checked_mul(64)?)?;
    for part in &plan.fragments {
        if !part.states.is_empty()
            || !part.shared_pools.is_empty()
            || !part.execution_regions.is_empty()
            || !part.execution_fusions.is_empty()
            || part
                .connections
                .iter()
                .any(|c| c.selected_line.is_some() || !c.admitted_lines.is_empty())
            || part.fore_ports.iter().any(|p| {
                p.selected_line.is_some()
                    || p.value_contract
                        .as_ref()
                        .is_some_and(|c| !c.constraints.is_empty())
            })
            || part
                .placements
                .iter()
                .any(|p| !p.terminal_transductions.is_empty())
        {
            return None;
        }
        bytes = bytes
            .checked_add(part.host_id.0.len())?
            .checked_add(part.fragment_id.0.len())?;
        bytes = bytes.checked_add(
            part.plan_fragments
                .len()
                .checked_mul(core::mem::size_of::<FragmentCommitment>())?,
        )?;
        for commitment in &part.plan_fragments {
            bytes = bytes
                .checked_add(commitment.host_id.0.len())?
                .checked_add(commitment.fragment_id.0.len())?;
        }
        bytes = bytes.checked_add(
            part.connections
                .len()
                .checked_mul(core::mem::size_of::<&PlannedConnection>())?,
        )?;
    }
    Some(bytes)
}
