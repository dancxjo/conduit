use super::*;
fn profiles() -> (KnownNetworkProfiles, KnownProfileKey, KnownProfileKey) {
    let mut profiles = KnownNetworkProfiles::new();
    let mut add = |name: &[u8]| {
        profiles
            .upsert(
                PrivateNetworkInput::new(
                    name,
                    b"public-test-password",
                    KnownSecurity::Wpa2Personal,
                    0,
                )
                .unwrap(),
            )
            .unwrap()
    };
    let a = add(b"first");
    let b = add(b"second");
    (profiles, a, b)
}
fn backend() -> KnownBackendAuth {
    KnownBackendAuth {
        wpa2_personal: true,
        wpa3_personal: false,
    }
}
#[test]
fn saved_failure_falls_back_without_scan_and_outcomes_are_exact() {
    let (profiles, a, b) = profiles();
    let mut policy = KnownNetworkPolicy::new();
    assert_eq!(
        policy.select_saved(&profiles, 0, None, backend()),
        Ok(KnownSelection::Attempt(a))
    );
    // Constructor pressure cannot reserve another attempt or change its profile.
    assert_eq!(
        policy.select_saved(&profiles, 1, Some(b), backend()),
        Ok(KnownSelection::AwaitOutcome(a))
    );
    assert_eq!(
        policy.outcome(b, 1, false),
        Err(KnownNetworkRefusal::Outcome)
    );
    policy.outcome(a, 1, false).unwrap();
    assert_eq!(
        policy.outcome(a, 1, false),
        Err(KnownNetworkRefusal::Outcome)
    );
    assert_eq!(
        policy.select_saved(&profiles, 1001, None, backend()),
        Ok(KnownSelection::Attempt(b))
    );
    assert_eq!(
        policy.outcome(a, 1002, false),
        Err(KnownNetworkRefusal::Outcome)
    );
    policy.outcome(b, 1002, true).unwrap();
}
#[test]
fn unissued_supersession_does_not_refund_attempts_or_accept_foreign_retirement() {
    let (profiles, a, b) = profiles();
    let mut policy = KnownNetworkPolicy::new();
    for now in 0..3 {
        assert_eq!(
            policy.select_saved(&profiles, now, Some(a), backend()),
            Ok(KnownSelection::Attempt(a))
        );
        assert_eq!(
            policy.retire_unissued(b, now),
            Err(KnownNetworkRefusal::Outcome)
        );
        policy.retire_unissued(a, now).unwrap();
        assert_eq!(
            policy.retire_unissued(a, now),
            Err(KnownNetworkRefusal::Outcome)
        );
    }
    assert_eq!(
        policy.select_saved(&profiles, 3, Some(a), backend()),
        Ok(KnownSelection::Exhausted)
    );
    assert_eq!(
        policy.select_saved(&profiles, 3, Some(b), backend()),
        Ok(KnownSelection::Attempt(b))
    );
}
#[test]
fn all_saved_profiles_exhaust_without_implicit_round_renewal() {
    let (profiles, a, b) = profiles();
    let mut policy = KnownNetworkPolicy::new();
    for round in 0..3 {
        let now = round * 60_000;
        assert_eq!(
            policy.select_saved(&profiles, now, None, backend()),
            Ok(KnownSelection::Attempt(a))
        );
        policy.outcome(a, now, false).unwrap();
        assert_eq!(
            policy.select_saved(&profiles, now, None, backend()),
            Ok(KnownSelection::Attempt(b))
        );
        policy.outcome(b, now, false).unwrap();
    }
    assert_eq!(
        policy.select_saved(&profiles, 1_000_000, None, backend()),
        Ok(KnownSelection::Exhausted)
    );
}
