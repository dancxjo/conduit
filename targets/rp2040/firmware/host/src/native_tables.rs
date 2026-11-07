//! Exact assigned tables shared by Root's native and mixed Back composition.
use super::{c, Refusal};
use conduit_kernel::assigned::{BoundedAssignedKernelTables, ResidentLayout};

pub fn prepare<const N: usize, const C: usize>(
    bytes: &[u8],
    maxima: c::AssignedPlanMaxima,
    requirements: c::AssignedPlanRequirements<'_>,
    computations: c::AssignedComputationProfile<'_>,
    fore: &[c::AssignedForeRequirement],
) -> Result<BoundedAssignedKernelTables<N, C, 1, N, C, N>, Refusal> {
    let view = c::decode_assigned_plan_with_fore(bytes, maxima, requirements, computations, fore)
        .map_err(|_| Refusal::Assignment)?;
    let mut pins = [c::AssignedRecordRequirement {
        tag: 0,
        payload_digest: [0; 32],
    }; 128];
    let mut count = 0;
    let schema = u16::from_le_bytes(bytes[8..10].try_into().map_err(|_| Refusal::Assignment)?);
    for record in c::AssignedPlanRecords::new(bytes).map_err(|_| Refusal::Assignment)? {
        let record = record.map_err(|_| Refusal::Assignment)?;
        if matches!(
            record.tag,
            c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
        ) {
            continue;
        }
        *pins.get_mut(count).ok_or(Refusal::Bounds)? =
            c::AssignedRecordRequirement::from_kernel_record(record, schema);
        count += 1;
    }
    BoundedAssignedKernelTables::prepare_with_fore(
        bytes,
        maxima,
        requirements,
        computations,
        &ResidentLayout {
            records: &pins[..count],
            counts: view.counts,
        },
        fore,
    )
    .map_err(Refusal::Tables)
}
