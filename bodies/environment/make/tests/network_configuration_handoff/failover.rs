use super::*;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_credential_failover.rs"]
mod production;
fn owner() -> (
    known::Owner,
    conduit_net::KnownProfileKey,
    conduit_net::KnownProfileKey,
) {
    let mut owner = known::Owner::empty(3);
    let a = owner
        .upsert_after_acknowledgement(input(b"a", b"public-first-pass"), |_| Ok(1))
        .unwrap();
    let b = owner
        .upsert_after_acknowledgement(input(b"b", b"public-second-pass"), |_| Ok(2))
        .unwrap();
    (owner, a, b)
}
#[test]
fn returned_failed_selection_alternates_without_consuming_later_ack() {
    let (mut owner, a, b) = owner();
    let mut policy = production::Fallback::new();
    let key = policy.choose(owner.profiles(), 0, None).unwrap();
    assert_eq!(key, a);
    let frozen = owner.freeze(key).unwrap();
    policy.frozen(frozen, 7).unwrap();
    assert!(policy.validates(frozen, 7));
    assert!(!policy.validates(frozen, 8));
    // Refused constructors keep exactly this source snapshot/reservation.
    assert_eq!(policy.choose(owner.profiles(), 1, None), Ok(a));
    assert_eq!(owner.selection(), Some(frozen));
    let mut wire = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    let actual = owner.take_selected(&mut wire).unwrap();
    policy.taken(actual).unwrap();
    let mut mailbox = NetworkSetupMailbox::new(1);
    let token = mailbox
        .submit(
            NetworkSetupInput::from_form(
                b"ssid=later&password=public-later-pass",
                NetworkSetupLimits {
                    minimum_credential_bytes: 8,
                    maximum_credential_bytes: 63,
                },
            )
            .unwrap(),
        )
        .unwrap();
    mailbox.acknowledge(token).unwrap();
    assert!(policy.settled(8, 2, Some((false, 10)), 11).is_err());
    assert!(policy.settled(7, 3, Some((false, 10)), 11).is_err());
    assert!(policy.supersede(11).is_err()); // actual input is already selected
    policy.settled(7, 2, Some((false, 10)), 60_000).unwrap();
    assert!(policy.settled(7, 2, Some((false, 10)), 60_000).is_err());
    assert_eq!(policy.choose(owner.profiles(), 60_000, None), Ok(b));
    assert_eq!(owner.store_revision(), 2);
    assert!(mailbox.acknowledged_submission() == Ok(token));
}
#[test]
fn actual_success_observation_settles_on_return_without_freshness_invention() {
    let (mut owner, a, _) = owner();
    let mut policy = production::Fallback::new();
    let key = policy.choose(owner.profiles(), 0, None).unwrap();
    assert_eq!(key, a);
    let frozen = owner.freeze(key).unwrap();
    policy.frozen(frozen, 1).unwrap();
    let mut wire = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    policy
        .taken(owner.take_selected(&mut wire).unwrap())
        .unwrap();
    assert!(policy.settled(1, 2, Some((true, 21)), 20).is_err());
    policy.settled(1, 2, Some((true, 10)), 60_000).unwrap();
    assert_eq!(policy.choose(owner.profiles(), 60_000, Some(a)), Ok(a));
}
#[test]
fn acknowledged_supersession_after_unused_return_does_not_refund_finite_attempts() {
    let (mut owner, a, b) = owner();
    let mut policy = production::Fallback::new();
    for sequence in 1..=3 {
        assert_eq!(policy.choose(owner.profiles(), sequence, Some(a)), Ok(a));
        let frozen = owner.freeze(a).unwrap();
        policy.frozen(frozen, 1).unwrap();
        // Caller already returned actual unused Ready before entering this seam.
        policy.supersede(sequence).unwrap();
        owner.retire_unconsumed(frozen).unwrap();
    }
    assert_eq!(
        policy.choose(owner.profiles(), 4, Some(a)),
        Err("credential-policy-exhausted")
    );
    assert_eq!(policy.choose(owner.profiles(), 4, Some(b)), Ok(b));
}
