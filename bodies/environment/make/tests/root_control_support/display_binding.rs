//! Production Root binding with pins that panic on electrical effects.
use super::*;

pub(super) fn check(
    assignment: &ValidatedAssignedDisplay,
    fragment: &c::PlanFragment,
    bytes: &[u8],
    records: &[c::AssignedRecord<'_>],
    profile: &AssignedDisplayProfile<'_>,
) {
    use conduit_environment_runtime::appliance::{
        assigned::ResidentSourceIdentity, assignment_transfer as t,
    };
    use native_display_binding::{bind, BindingInput};
    let resident: Vec<_> = records
        .iter()
        .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
        .copied()
        .collect();
    let mut wire = vec![0; t::MAXIMUM_WIRE_BYTES];
    let size = t::encode(
        &bytes[..125],
        records,
        ResidentSourceIdentity::from_expanded(&profile.source.expanded_plot_id),
        &resident,
        &mut wire,
    )
    .unwrap();
    wire.truncate(size);
    fixtures::scope_mode(3);
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    fixtures::scope_mode(0);
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    issuer
        .reserve_display_play_matching(
            assignment,
            &fragment.plan_id,
            &fragment.fragment_id,
            active.active_play_id.as_str(),
        )
        .unwrap();
    let mut owner = Some(display_fixture::ready(&fragment.host_id, &fragment.boot_id));
    let input = |available_bytes| BindingInput {
        assignment,
        wire: &wire,
        profile,
        resident: &resident,
        fragment: &fragment.fragment_id,
        active: &active,
        maximum_frames: 2,
        available_bytes,
    };
    assert!(bind(&mut issuer, &mut owner, input(0)).is_err());
    assert!(owner.is_some());
    let bound = bind(&mut issuer, &mut owner, input(usize::MAX)).unwrap();
    assert!(owner.is_none());
    assert_eq!(bound.inspection().completed_operations, 0);
    assert_eq!(
        bound.inspection().scope.active_play_id,
        active.active_play_id
    );
    assert!(bind(&mut issuer, &mut owner, input(usize::MAX)).is_err());
}
