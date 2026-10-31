use super::*;

fn station() -> Recovery {
    let mut recovery = Recovery::new();
    recovery.configuration(7, 10).unwrap();
    assert_eq!(
        recovery.poll(10, true, false),
        Ok(Action::PrepareAttempt {
            revision: 7,
            generation: 1
        })
    );
    recovery.attached(1).unwrap();
    recovery
}
fn identity(recovery: &Recovery) -> (u64, u64, u8) {
    (recovery.revision, recovery.generation, recovery.attempts)
}
#[test]
fn acknowledged_station_intent_retires_once_without_accepting_or_spending() {
    let mut recovery = station();
    let original = identity(&recovery);
    assert_eq!(recovery.poll(20, true, true), Ok(Action::None));
    assert_eq!(recovery.configuration(8, 20), Err(Refusal::Busy));
    assert_eq!(
        recovery.configuration_retirement(20),
        Ok(Action::RestoreAccessPoint { generation: 1 })
    );
    assert_eq!(identity(&recovery), original);
    for now in [20, 21, 1_000, 90_000] {
        assert_eq!(recovery.configuration_retirement(now), Ok(Action::None));
        assert_eq!(recovery.poll(now, true, true), Ok(Action::None));
        assert!(recovery.is_restoring());
        assert!(!recovery.accepts_configuration());
        assert_eq!(recovery.configuration(8, now), Err(Refusal::Busy));
        assert_eq!(identity(&recovery), original);
    }
    // Only an actual successful cleanup/retirement report opens the existing
    // configuration entrance; the new intent has not been consumed by policy.
    recovery.restored(1, 90_000, true).unwrap();
    assert_eq!(identity(&recovery), original);
    recovery.configuration(8, 90_000).unwrap();
    assert_eq!(recovery.revision(), 8);
    assert_eq!(recovery.generation(), 1);
    assert_eq!(
        recovery.poll(90_000, true, false),
        Ok(Action::PrepareAttempt {
            revision: 8,
            generation: 2
        })
    );
}
#[test]
fn joining_is_busy_and_keeps_the_original_attempt_until_it_finishes() {
    let mut recovery = Recovery::new();
    recovery.configuration(7, 10).unwrap();
    recovery.poll(10, true, false).unwrap();
    let original = identity(&recovery);
    for now in [10, 20, 30] {
        assert_eq!(recovery.configuration_retirement(now), Err(Refusal::Busy));
        assert_eq!(identity(&recovery), original);
        assert!(recovery.is_joining());
    }
    recovery.attached(1).unwrap();
    assert_eq!(
        recovery.configuration_retirement(30),
        Ok(Action::RestoreAccessPoint { generation: 1 })
    );
}
#[test]
fn stale_clock_cannot_request_retirement_or_change_currentness() {
    let mut recovery = station();
    recovery.poll(100, false, true).unwrap();
    let original = identity(&recovery);
    assert_eq!(recovery.configuration_retirement(99), Err(Refusal::Clock));
    assert_eq!(recovery.phase, Phase::Station);
    assert_eq!(recovery.last_now, 100);
    assert_eq!(identity(&recovery), original);
    assert_eq!(
        recovery.configuration_retirement(100),
        Ok(Action::RestoreAccessPoint { generation: 1 })
    );
}
#[test]
fn failed_cleanup_never_accepts_configuration_or_restarts_retirement() {
    let mut recovery = station();
    recovery.configuration_retirement(20).unwrap();
    let original = identity(&recovery);
    recovery.restored(1, 21, false).unwrap();
    for now in [21, 1_000, 90_000] {
        assert_eq!(recovery.configuration_retirement(now), Err(Refusal::Busy));
        assert_eq!(recovery.poll(now, true, false), Ok(Action::None));
        assert_eq!(recovery.configuration(8, now), Err(Refusal::Busy));
        assert_eq!(identity(&recovery), original);
        assert!(!recovery.accepts_configuration());
    }
}
#[test]
fn returned_or_unselected_configuration_needs_no_station_retirement() {
    let mut recovery = Recovery::new();
    assert_eq!(recovery.configuration_retirement(0), Ok(Action::None));
    recovery.configuration(7, 10).unwrap();
    assert_eq!(recovery.configuration_retirement(10), Ok(Action::None));
    assert_eq!(recovery.generation(), 0);
    assert_eq!(recovery.attempts, 0);
    recovery.configuration(8, 10).unwrap();
    assert_eq!(recovery.revision(), 8);
}
