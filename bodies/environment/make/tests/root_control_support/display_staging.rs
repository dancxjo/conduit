//! Production transfer and dormant kernel preparation against finite backing.
use super::*;

pub(super) fn check(
    assignment: &ValidatedAssignedDisplay,
    bytes: &[u8],
    records: &[c::AssignedRecord<'_>],
    profile: &AssignedDisplayProfile<'_>,
    host: &c::HostAdvertisement,
    software: &mut conduit_environment_runtime::appliance::software::ReadySoftwareBacks,
) {
    use conduit_environment_runtime::appliance::{
        assigned::ResidentSourceIdentity, assignment_transfer as t,
    };
    use native_display_assignment::StagedDisplayAssignment;
    let resident: Vec<_> = records
        .iter()
        .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
        .copied()
        .collect();
    let source = ResidentSourceIdentity::from_expanded(&profile.source.expanded_plot_id);
    let mut wire = vec![0; t::MAXIMUM_WIRE_BYTES];
    let size = t::encode(&bytes[..125], records, source, &resident, &mut wire).unwrap();
    wire.truncate(size);
    let scope = || AssignedDisplayScope {
        host: &host.host_id,
        boot: &host.boot_id,
        generation: host.offer_generation,
    };
    assert!(StagedDisplayAssignment::stage(&wire, profile, &resident, scope(), 0).is_err());
    let foreign: c::BootId = "foreign/boot".into();
    assert!(StagedDisplayAssignment::stage(
        &wire,
        profile,
        &resident,
        AssignedDisplayScope {
            boot: &foreign,
            ..scope()
        },
        usize::MAX
    )
    .is_err());
    let staged =
        StagedDisplayAssignment::stage(&wire, profile, &resident, scope(), usize::MAX).unwrap();
    assert_eq!(staged.admitted.identity(), assignment.identity());
    assert_eq!(staged.seal(), c::assigned_plan_payload_digest(&wire));
    assert_eq!(staged.wire(), wire);
    assert!(native_display_kernel::prepare(
        staged.wire(),
        profile,
        &resident,
        scope(),
        software,
        2,
        0
    )
    .is_err());
    let needed = native_display_kernel::requirements().unwrap();
    let (refused, usage) = allocation_probe::observe(|| {
        native_display_kernel::prepare(
            staged.wire(),
            profile,
            &resident,
            scope(),
            software,
            2,
            needed - 1,
        )
    });
    assert!(refused.is_err());
    assert_eq!((usage.allocations, usage.reallocations), (0, 0));
    let (prepared, usage) = allocation_probe::observe(|| {
        native_display_kernel::prepare(
            staged.wire(),
            profile,
            &resident,
            scope(),
            software,
            2,
            needed,
        )
        .unwrap()
    });
    assert!(
        usage.requested_bytes <= needed,
        "{} requested against {needed} admitted",
        usage.requested_bytes
    );
    assert_eq!(prepared.identity, assignment.identity());
    assert_eq!(prepared.input, assignment.fore()[0]);
    // Replaying preparation cannot reconstruct withdrawn portable owners.
    assert!(native_display_kernel::prepare(
        staged.wire(),
        profile,
        &resident,
        scope(),
        software,
        2,
        usize::MAX
    )
    .is_err());
    wire[12] ^= 1;
    assert!(
        StagedDisplayAssignment::stage(&wire, profile, &resident, scope(), usize::MAX).is_err()
    );
    assert_ne!(staged.wire(), wire);
}
