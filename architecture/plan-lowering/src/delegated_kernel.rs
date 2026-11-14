//! Approved topology to the existing kernel's finite active-prefix tables.
use conduit_body::{AssignedBodyPreparationProfile, DelegatedBodyFragment};
use conduit_core as c;
use conduit_kernel::assigned::{
    AssignedKernelRefusal, BoundedAssignedKernelTables, ResidentLayout,
};

/// The installed delegation has approved this exact topology. Local computation,
/// Fore and capacity pins still pass through the ordinary Core/kernel checks.
/// No resources, native handles, provider loans or start authority are issued.
/// Root must retain current-Boot approval/workspace custody and independently
/// admit the complete local workset before borrowing providers or starting.
pub fn prepare_delegated_body_kernel_tables<
    const RECORDS: usize,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    fragment: &DelegatedBodyFragment<'_>,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<BoundedAssignedKernelTables<N, C, P, R, T, H>, AssignedKernelRefusal> {
    if RECORDS > 256 {
        return Err(AssignedKernelRefusal::Capacity);
    }
    let mut pins = [c::AssignedRecordRequirement {
        tag: 0,
        payload_digest: [0; 32],
    }; RECORDS];
    let mut count = 0;
    let projection = fragment.projection();
    let schema = u16::from_le_bytes(
        projection[8..10]
            .try_into()
            .map_err(|_| AssignedKernelRefusal::Layout)?,
    );
    for record in c::AssignedPlanRecords::new(projection)? {
        let record = record?;
        if matches!(
            record.tag,
            c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
        ) {
            continue;
        }
        *pins.get_mut(count).ok_or(AssignedKernelRefusal::Capacity)? =
            c::AssignedRecordRequirement::from_kernel_record(record, schema);
        count += 1;
    }
    BoundedAssignedKernelTables::prepare_with_fore(
        projection,
        profile.maxima,
        profile.requirements,
        profile.computations,
        &ResidentLayout {
            records: &pins[..count],
            counts: fragment.view().counts,
        },
        profile.fore.ok_or(AssignedKernelRefusal::Layout)?,
    )
}
