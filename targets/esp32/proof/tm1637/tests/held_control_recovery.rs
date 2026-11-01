//! Actual held executor and recovery policy under inert CPU providers.
//! This does not exercise Root workspace scheduling, flash or physical cleanup.
use super::*;
use crate::{
    native_network_recovery::{Action, Recovery, Refusal},
    support::{held_fixture, immediate},
};

#[test]
fn unreleased_selection_retires_at_original_deadline_without_consuming_credentials() {
    let (mut play, state, mut issuer) = held_fixture();
    let active = play.active().unwrap().clone();
    let kernel = &**play.prepared.as_ref().unwrap() as *const PreparedNetworkKernel;
    let mut recovery = Recovery::new();
    recovery.configuration(1, 0).unwrap();
    let candidate = recovery.preview_attempt(0).unwrap().unwrap();
    recovery.begin_attempt(candidate, 0).unwrap();
    for now in [0, 1, 4_000, 14_999] {
        immediate(play.run());
        assert_eq!(play.status(), "installed-held");
        assert_eq!(play.active(), Some(&active));
        assert_eq!(&**play.prepared.as_ref().unwrap() as *const _, kernel);
        assert!(!play.permits_unselected_configuration());
        assert_eq!(state.calls.get(), [0, 0, 0]);
        assert_eq!(recovery.poll(now, false, false), Ok(Action::None));
        assert!(recovery.preview_attempt(now).unwrap().is_none());
        assert_eq!(recovery.configuration(2, now), Err(Refusal::Busy));
    }
    assert_eq!(
        recovery.poll(15_000, false, false),
        Ok(Action::RestoreAccessPoint { generation: 1 })
    );
    assert_eq!(recovery.poll(15_001, false, false), Ok(Action::None));
    assert!(!recovery.attempt_due(15_001));
    immediate(play.restore_for_retirement()).unwrap();
    // An uninvoked join has no radio cleanup effect to perform.
    assert_eq!(state.calls.get(), [0, 0, 0]);
    let mut retired = play.take_retired().unwrap();
    assert_eq!(retired.active(), &active);
    assert!(retired
        .evidence()
        .events()
        .any(|event| matches!(event.kind, k::KernelEventKind::RunCancelled)));
    // Cleanup alone cannot acknowledge durable retirement or return owners.
    assert!(!retired.evidence_is_committed());
    assert!(retired.take_owners().is_none());
    assert!(issuer.finish_network_play(&retired).is_err());
    assert_eq!(state.profile_updates.get(), 0);
    assert!(recovery.is_restoring());
    assert!(!recovery.attempt_due(60_000));
    // An uninvoked join has no radio cleanup effect to perform.
    assert_eq!(state.calls.get(), [0, 0, 0]);
}
