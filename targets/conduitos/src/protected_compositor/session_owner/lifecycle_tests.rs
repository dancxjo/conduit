use super::*;

#[test]
fn retained_activation_records_same_instance_and_only_actual_reset_entries() {
    let mut owner = SessionOwner::new(Fake::new(7)).unwrap();
    let first = owner.activate().unwrap();
    let second = owner.activate_withdrawn().unwrap();
    let observation = owner.activation_observation();
    assert_eq!(observation.attempts.count, 2);
    assert!(!observation.attempts.overflowed);
    assert_eq!(observation.instance_before, 7);
    assert_eq!(observation.instance_after, 7);
    assert_eq!(observation.namespace_before, 1);
    assert_eq!(observation.namespace_after, 2);
    let sample = observation.attempts.last.unwrap();
    assert!(sample.succeeded);
    // The fake has no measured backend costs; absence must not become zero.
    assert_eq!(sample.before, None);
    assert_eq!(sample.after, None);
    assert!(!owner.is_active(first.session));
    assert!(owner.is_active(second.session));
    assert_eq!(owner.transport.commands.len(), 4);
}

#[test]
fn failed_activation_remains_terminal_without_observer_retry() {
    let mut transport = Fake::new(9);
    transport.fail_at = Some(1);
    let mut owner = SessionOwner::new(transport).unwrap();
    assert_eq!(owner.activate(), Err(OwnerError::Transport(99)));
    assert!(owner.is_terminated());
    assert!(
        !owner
            .activation_observation()
            .attempts
            .last
            .unwrap()
            .succeeded
    );
    assert_eq!(owner.activation_observation().namespace_after, 0);
    assert_eq!(owner.activate_withdrawn(), Err(OwnerError::Terminated));
    assert_eq!(owner.activation_observation().attempts.count, 2);
    assert_eq!(owner.transport.commands.len(), 1);
    assert_eq!(owner.transport.quarantines, 1);
}
