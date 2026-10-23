//! Actual Root issuer with CPU-admitted tables; no authenticated device start.
use super::*;
pub(super) fn check(
    assignment: &conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance,
    fragment: &c::PlanFragment,
) {
    for mode in [1, 2] {
        fixtures::scope_mode(mode);
        let mut foreign = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
        fixtures::scope_mode(0);
        let (result, memory) = allocation_probe::observe(|| {
            foreign.reserve_assigned_play(assignment, &fragment.plan_id, &fragment.fragment_id)
        });
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    let first = issuer.reserve_play(fragment).unwrap();
    for (plan, part) in [
        (
            c::PlanId::from("foreign/plan"),
            fragment.fragment_id.clone(),
        ),
        (
            fragment.plan_id.clone(),
            c::FragmentId::from("foreign/fragment"),
        ),
    ] {
        let (result, memory) =
            allocation_probe::observe(|| issuer.reserve_assigned_play(assignment, &plan, &part));
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    // Refusals preserve both the sequence and the existing reserved scope.
    let native = fixtures::identity(false);
    issuer
        .issue(
            &native,
            native_authority::Operation::Credentials,
            fragment,
            &first,
        )
        .unwrap();
    let second = issuer
        .reserve_assigned_play(assignment, &fragment.plan_id, &fragment.fragment_id)
        .unwrap();
    assert_eq!(second.play_sequence, first.play_sequence + 1);
    assert_eq!(
        second,
        c::bind_active_play(
            &fragment.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            second.play_sequence,
        )
    );
    let third = issuer.reserve_play(fragment).unwrap();
    assert_eq!(third.play_sequence, second.play_sequence + 1);
    assert!(issuer
        .issue(
            &fixtures::identity(true),
            native_authority::Operation::Join,
            fragment,
            &second
        )
        .is_err());
    issuer
        .issue(
            &fixtures::identity(true),
            native_authority::Operation::Join,
            fragment,
            &third,
        )
        .unwrap();
}
