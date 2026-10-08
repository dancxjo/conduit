//! Before-allocation reservation for the complete fixed pure Source Plan verifier.
//! Target construction/storage is a separate external contract.
use crate::{
    parser_session_execution::ParserSessionEntry,
    parser_session_numeric_plan_storage::{numeric_plan_retained_bytes, NumericPlanStorageRefusal},
};
use conduit_core::Plan;
#[derive(Clone, Copy, Debug)]
pub(crate) struct SourcePlanPreparationReservation {
    pub(crate) original_plan_retained_bytes: usize,
    pub(crate) temporary_requested_bytes_bound: usize,
}
pub(crate) fn source_plan_preparation_reservation(
    plan: &Plan,
    entry: ParserSessionEntry,
    source_verification_preparation_bytes_bound: usize,
    maximum_temporary_requested_bytes: usize,
) -> Result<SourcePlanPreparationReservation, NumericPlanStorageRefusal> {
    use NumericPlanStorageRefusal as R;
    let original = numeric_plan_retained_bytes(plan)?;
    let count = entry.program_hex().lines().count();
    let f = &plan.fragments[0];
    if !(1..=64).contains(&count)
        || f.placements.len() != count
        || f.connections.len() != count - 1
        || f.fore_ports.len() != 2
        || f.execution_regions.len() > count
        || f.startup_dependencies.len() > count.checked_mul(count).ok_or(R::Overflow)?
        || f.startup_order.len() != count
        || f.expected_sign.len() > 256
        || f.expected_terminals.len() > 128
        || f.plan_fragments.len() != 1
        || f.execution_regions
            .iter()
            .any(|region| region.admitted_placements.len() > count)
        || f.placements
            .iter()
            .any(|gear| gear.host_calls.len() > 16 || gear.resources.len() > 1)
    {
        return Err(R::Unsupported);
    }
    // Complete allocation-capacity traversal supplies the original Plan term.
    // As for the fixed numerical verifier, 128x covers repeated canonical
    // fingerprint fields, framing expansion and cumulative geometric buffers.
    // The entire already-admitted Source verifier preparation envelope covers
    // all program decode/Type trees; 64x separately reserves definition/seal
    // reconstruction and its nested encodings. Fixed-layout sorting/definition
    // scaffolding is charged independently for every possible Gear.
    let temporary = original
        .checked_mul(128)
        .and_then(|n| {
            source_verification_preparation_bytes_bound
                .checked_mul(64)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            count
                .checked_mul(8 * 1024 * 1024)
                .and_then(|m| n.checked_add(m))
        })
        .ok_or(R::Overflow)?;
    if temporary > maximum_temporary_requested_bytes {
        return Err(R::Pressure);
    }
    Ok(SourcePlanPreparationReservation {
        original_plan_retained_bytes: original,
        temporary_requested_bytes_bound: temporary,
    })
}
