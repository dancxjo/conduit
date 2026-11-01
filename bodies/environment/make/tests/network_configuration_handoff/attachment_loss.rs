//! CPU observation/policy proof; no physical radio or durable Sign claim.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_station_outcome.rs"]
mod observation;
use super::{input, known};
use observation::StationOutcome;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_credential_failover.rs"]
mod production;

#[test]
fn later_loss_uses_first_loss_clock_within_previous_state_budget() {
    let mut outcome = StationOutcome::attempt(Ok(()), 10);
    outcome.detached(9);
    assert_eq!(outcome.policy_outcome(), Some((true, 10)));
    outcome.detached(20);
    outcome.detached(30);
    assert_eq!(outcome.policy_outcome(), Some((false, 20)));
    assert!(
        core::mem::size_of::<StationOutcome>()
            <= core::mem::size_of::<Option<(Result<(), conduit_net::StationAttemptFailure>, u64)>>(
            )
    );
}

#[test]
fn loss_cannot_invent_an_attempt_or_replace_an_actual_join_failure() {
    let mut absent = StationOutcome::default();
    absent.detached(50);
    assert_eq!(absent.policy_outcome(), None);
    let failure = conduit_net::StationAttemptFailure {
        phase: conduit_net::StationPhase::Associate,
        reason: conduit_net::StationFailure::Rejected,
    };
    let mut failed = StationOutcome::attempt(Err(failure), 10);
    failed.detached(20);
    assert_eq!(failed.policy_outcome(), Some((false, 10)));
}

#[test]
fn joined_then_lost_profile_yields_to_other_saved_network_after_custody_return() {
    let mut owner = known::Owner::empty(3);
    let first = owner
        .upsert_after_acknowledgement(input(b"first", b"public-first-pass"), |_| Ok(1))
        .unwrap();
    let second = owner
        .upsert_after_acknowledgement(input(b"second", b"public-second-pass"), |_| Ok(2))
        .unwrap();
    let mut policy = production::Fallback::new();
    assert_eq!(policy.choose(owner.profiles(), 0, None), Ok(first));
    let frozen = owner.freeze(first).unwrap();
    policy.frozen(frozen, 7).unwrap();
    let mut wire = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    policy
        .taken(owner.take_selected(&mut wire).unwrap())
        .unwrap();
    let mut observed = StationOutcome::attempt(Ok(()), 10);
    observed.detached(20);
    assert!(policy.settled(8, 2, observed.policy_outcome(), 21).is_err());
    policy.settled(7, 2, observed.policy_outcome(), 21).unwrap();
    assert_eq!(policy.choose(owner.profiles(), 21, None), Ok(second));
}
