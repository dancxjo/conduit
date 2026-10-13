//! Complete original Plan storage using the shared supported Core inventory.
//! Ordinary verifier temporary requests remain separately pre-admitted below.
use conduit_core::{Plan, PlanStorageRefusal};
use core::mem::{align_of, size_of};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericPlanStorageRefusal {
    Overflow,
    Unsupported,
    Pressure,
}
/// Complete supported owned capacities plus the retained Rc allocation payload
/// and conservative header/alignment; shared owners are charged once upstream.
pub(crate) fn numeric_plan_retained_bytes(plan: &Plan) -> Result<usize, NumericPlanStorageRefusal> {
    let heap = conduit_core::plan_owned_heap_bytes(plan).map_err(|refusal| match refusal {
        PlanStorageRefusal::Overflow => NumericPlanStorageRefusal::Overflow,
        PlanStorageRefusal::Unsupported => NumericPlanStorageRefusal::Unsupported,
    })?;
    heap.checked_add(size_of::<Plan>())
        .and_then(|n| n.checked_add(2 * size_of::<usize>()))
        .and_then(|n| n.checked_add(4 * align_of::<Plan>()))
        .ok_or(NumericPlanStorageRefusal::Overflow)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NumericPlanPreparationReservation {
    pub(crate) original_plan_retained_bytes: usize,
    pub(crate) temporary_requested_bytes_bound: usize,
}
/// Preflight for the fixed three-Gear verifier, not target preparation. The
/// caller separately reserves the original Source evaluators and model owners.
/// All shape rejection and arithmetic precede the ordinary allocating verifiers.
pub(crate) fn numeric_plan_preparation_reservation(
    plan: &Plan,
    model_retained_bytes: usize,
    source_preparation_bytes: [usize; 2],
    maximum_temporary_requested_bytes: usize,
) -> Result<NumericPlanPreparationReservation, NumericPlanStorageRefusal> {
    use NumericPlanStorageRefusal as R;
    let original = numeric_plan_retained_bytes(plan)?;
    let f = &plan.fragments[0];
    if f.placements.len() != 3
        || f.connections.len() != 2
        || f.fore_ports.len() != 2
        || f.execution_regions.len() > 3
        || f.startup_dependencies.len() > 6
        || f.startup_order.len() != 3
        || f.expected_sign.len() > 16
        || f.expected_terminals.len() > 8
        || f.plan_fragments.len() != 1
        || f.execution_regions
            .iter()
            .any(|r| r.admitted_placements.len() > 3)
        || f.placements
            .iter()
            .any(|g| g.host_calls.len() > 16 || g.resources.len() > 1)
    {
        return Err(R::Unsupported);
    }
    // One fragment fingerprint and one Plan fingerprint, fixed-size temporary
    // sorting/commitment owners, and their geometric canonical Vec requests.
    // Complete Plan allocation sizes cover every string and encoded field;
    // 128x admits repeated canonical fields, <=8x primitive framing expansion
    // and <=4x cumulative Vec growth with independent overlap allowance.
    // The two fixed pure definitions re-encode their complete programs/Types;
    // their already measured structural preparation envelopes are charged64x.
    // Model offer reconstruction is charged independently from the full model
    // owner's retained receipt. The fixed 8MiB covers fixed-layout scaffolding.
    let temporary = original
        .checked_mul(128)
        .and_then(|n| {
            model_retained_bytes
                .checked_mul(64)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            source_preparation_bytes[0]
                .checked_add(source_preparation_bytes[1])
                .and_then(|m| m.checked_mul(64))
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(8 * 1024 * 1024))
        .ok_or(R::Overflow)?;
    if temporary > maximum_temporary_requested_bytes {
        return Err(R::Pressure);
    }
    Ok(NumericPlanPreparationReservation {
        original_plan_retained_bytes: original,
        temporary_requested_bytes_bound: temporary,
    })
}
