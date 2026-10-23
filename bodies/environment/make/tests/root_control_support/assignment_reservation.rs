//! Actual Root issuer with CPU-admitted tables; no authenticated device start.
use super::*;
pub(super) fn check(
    assignment: &conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance,
    fragment: &c::PlanFragment,
) {
    signed_reservation(assignment, fragment);
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

fn signed_reservation(
    assignment: &conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance,
    fragment: &c::PlanFragment,
) {
    use ed25519_dalek::SigningKey;
    use native_owner_control::{Operation, OwnerControl, Request, PROTOCOL};
    let key = SigningKey::from_bytes(&[11; 32]);
    let expected = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let seal = [42; 32];
    for failure in 0..5 {
        let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
        assert_eq!(issuer.next_play_sequence(), Some(1));
        let mut gate = OwnerControl::new(Some(key.verifying_key().to_bytes())).unwrap();
        let wrong_plan: c::PlanId = "wrong/plan".into();
        let wrong_fragment: c::FragmentId = "wrong/fragment".into();
        let make = |sequence, bad| Request {
            protocol: PROTOCOL,
            operation: if bad && failure == 0 {
                Operation::Start
            } else {
                Operation::Reserve
            },
            sequence,
            host: fragment.host_id.as_str(),
            boot: fragment.boot_id.as_str(),
            offer_generation: 1,
            body: "body/owner",
            wake: "wake/owner",
            body_plan: "body-plan/owner",
            body_play: "body-play/owner",
            plan: if bad && failure == 2 {
                wrong_plan.as_str()
            } else {
                fragment.plan_id.as_str()
            },
            fragment: if bad && failure == 3 {
                wrong_fragment.as_str()
            } else {
                fragment.fragment_id.as_str()
            },
            fragment_play: if bad && failure == 4 {
                "play/unreserved"
            } else {
                expected.active_play_id.as_str()
            },
            assignment_seal: if bad && failure == 1 { [43; 32] } else { seal },
        };
        let token = authenticate(&mut gate, &key, make(1, true), fragment);
        assert!(native_owner_reservation::OwnerReservation::reserve(
            token,
            assignment,
            &seal,
            &mut issuer
        )
        .is_err());
        let token = authenticate(&mut gate, &key, make(2, false), fragment);
        let reserved = native_owner_reservation::OwnerReservation::reserve(
            token,
            assignment,
            &seal,
            &mut issuer,
        )
        .unwrap();
        assert_eq!(issuer.next_play_sequence(), Some(2));
        assert_eq!(reserved.active, expected); // refusal consumed no Root reservation
        assert_eq!(reserved.fragment, fragment.fragment_id);
        assert_eq!(reserved.assignment_seal, seal);
        assert_eq!(
            (
                &*reserved.body,
                &*reserved.wake,
                &*reserved.body_plan,
                &*reserved.body_play
            ),
            (
                "body/owner",
                "wake/owner",
                "body-plan/owner",
                "body-play/owner"
            )
        );
    }
}

fn authenticate<'a>(
    gate: &mut native_owner_control::OwnerControl,
    key: &ed25519_dalek::SigningKey,
    request: native_owner_control::Request<'a>,
    fragment: &c::PlanFragment,
) -> native_owner_control::AuthenticatedRequest<'a> {
    use ed25519_dalek::Signer;
    let signature = key.sign(&request.transcript().unwrap()).to_bytes();
    gate.authenticate(
        request,
        &signature,
        fragment.host_id.as_str(),
        fragment.boot_id.as_str(),
        1,
    )
    .unwrap()
}
