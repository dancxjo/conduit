//! Ordinary lifecycle transitions with explicit CPU membership/start fixtures.
//! These tests do not admit a live browser, storage provider or execution Line.
use super::*;
use conduit_body::{BodyLifecycleSessionError, BodyPlayIdentity, WakeLifecycle};
use conduit_environment_make::{NativeBodyBinding, NativeBodyBindingRefusal as E};
#[path = "../support/body.rs"]
pub(super) mod support;
fn prepared() -> PreparedRecordingRecovery {
    let mut first = reference();
    first.lifetime.version = c::ResourceVersionIdentity::from_digest([1; 32]);
    let mut second = first.clone();
    second.lifetime.version = c::ResourceVersionIdentity::from_digest([2; 32]);
    let fixture = batch_plan::fixture(&[first, second]);
    PreparedRecordingRecovery::prepare_batch(
        &fixture.source,
        &fixture.host,
        &fixture.choices,
        &bases(),
        &fixture.grants,
    )
    .unwrap()
}
#[test]
fn recorder_proposal_and_fragment_identity_require_the_retained_body_start() {
    let recorder = prepared();
    let fragment = &recorder.plan().fragments[0];
    let mut owner = support::body_owner(
        recorder.resident_plot(),
        &fragment.host_id,
        &fragment.boot_id,
        false,
    );
    let proposal = recorder.propose_to_body(&mut owner, vec![]).unwrap();
    assert_eq!(proposal.wake.lifecycle, WakeLifecycle::AwaitingPlan);
    assert!(proposal.play.is_none());
    assert_eq!(proposal.plan.plots[0].plan, *recorder.plan());
    let body_play = BodyPlayIdentity::bind(&proposal.plan, 1);
    let fragment_play =
        c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 7);
    assert_ne!(body_play.active_play_id, fragment_play.active_play_id);
    let mut binding =
        NativeBodyBinding::prepare_recording(&recorder, &owner, &body_play, &fragment_play)
            .unwrap();
    assert_eq!(binding.fragment_play(&owner), Err(E::NotStarted));
    assert_eq!(binding.confirm_started(&owner), Err(E::NotStarted));
    support::start_body(&mut owner, &body_play, &fragment.host_id, &fragment.boot_id);
    binding.confirm_started(&owner).unwrap();
    assert_eq!(binding.confirm_started(&owner), Err(E::AlreadyConfirmed));
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(binding.fragment_play(&owner).unwrap(), &fragment_play);
        assert_eq!(binding.body_play(&owner).unwrap(), &body_play);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    owner
        .lull(&fragment.host_id, &fragment.boot_id, Some(&body_play))
        .unwrap();
    assert_eq!(binding.fragment_play(&owner), Err(E::StaleProposal));
    binding.revoke();
    assert_eq!(binding.fragment_play(&owner), Err(E::Revoked));
    owner.evidence().validate().unwrap();
}
#[test]
fn recorder_cannot_propose_to_foreign_membership_or_substitute_a_fragment_play() {
    let recorder = prepared();
    let fragment = &recorder.plan().fragments[0];
    let mut stale = support::body_owner(
        recorder.resident_plot(),
        &fragment.host_id,
        &"fixture/stale-boot".into(),
        false,
    );
    let before = stale.evidence().clone();
    assert!(matches!(
        recorder.propose_to_body(&mut stale, vec![]),
        Err(BodyLifecycleSessionError::StaleHost)
    ));
    assert_eq!(stale.evidence(), &before);
    assert!(stale.realization().is_none());
    let mut wrong = support::body_owner(
        recorder.resident_plot(),
        &fragment.host_id,
        &fragment.boot_id,
        true,
    );
    let before = wrong.evidence().clone();
    assert!(matches!(
        recorder.propose_to_body(&mut wrong, vec![]),
        Err(BodyLifecycleSessionError::Plan(_))
    ));
    assert_eq!(wrong.evidence(), &before);
    let mut owner = support::body_owner(
        recorder.resident_plot(),
        &fragment.host_id,
        &fragment.boot_id,
        false,
    );
    let body_play = BodyPlayIdentity::bind(
        &recorder.propose_to_body(&mut owner, vec![]).unwrap().plan,
        1,
    );
    let fragment_play =
        c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 7);
    for field in [0, 1, 2, 3] {
        let mut foreign = fragment_play.clone();
        match field {
            0 => foreign.boot_id = "fixture/foreign-boot".into(),
            1 => foreign.host_id = "fixture/foreign-host".into(),
            2 => foreign.plan_id = "fixture/foreign-plan".into(),
            _ => foreign.active_play_id = "fixture/foreign-play".into(),
        }
        assert!(matches!(
            NativeBodyBinding::prepare_recording(&recorder, &owner, &body_play, &foreign),
            Err(E::WrongFragment)
        ));
    }
    let mut binding =
        NativeBodyBinding::prepare_recording(&recorder, &owner, &body_play, &fragment_play)
            .unwrap();
    binding.revoke();
    support::start_body(&mut owner, &body_play, &fragment.host_id, &fragment.boot_id);
    assert_eq!(binding.confirm_started(&owner), Err(E::Revoked));
}
