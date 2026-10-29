use super::*;
#[path = "tests/observed_round.rs"]
mod observed_round;
fn input(ssid: &[u8], priority: i8) -> PrivateNetworkInput {
    PrivateNetworkInput::new(
        ssid,
        b"test-passphrase",
        KnownSecurity::Wpa2Personal,
        priority,
    )
    .unwrap()
}
fn scan(ssid: &[u8], rssi: i16, now: u64) -> KnownScan<'_> {
    KnownScan {
        ssid,
        security: KnownSecurity::Wpa2Personal,
        rssi_dbm: rssi,
        observed_ms: now,
    }
}
#[test]
fn four_profiles_atomic_capacity_replacement_security_and_secret_selection() {
    let mut p = KnownNetworkProfiles::new();
    assert!(p.is_empty());
    let first = p.upsert(input(b"a", 0)).unwrap();
    for name in [b"b", b"c", b"d"] {
        p.upsert(input(name, 0)).unwrap();
    }
    assert_eq!(p.len(), MAXIMUM_KNOWN_NETWORKS);
    assert_eq!(p.upsert(input(b"e", 0)), Err(KnownNetworkRefusal::Capacity));
    p.with_request(first, |r| {
        assert_eq!(r.ssid, b"a");
        assert_eq!(r.credential, b"test-passphrase");
    })
    .unwrap();
    let next = p
        .upsert(
            PrivateNetworkInput::new(b"a", b"replacement-pass", KnownSecurity::Wpa3Personal, 9)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(next.slot, first.slot);
    assert!(next.generation > first.generation);
    assert_eq!(p.len(), 4);
    assert_eq!(p.info(first), Err(KnownNetworkRefusal::StaleProfile));
    assert!(p.matches(next, b"a", KnownSecurity::Wpa3Personal));
    p.with_request(next, |r| assert_eq!(r.credential, b"replacement-pass"))
        .unwrap();
    assert!(PrivateNetworkInput::new(b"a", b"short", KnownSecurity::Wpa2Personal, 0).is_err());
    assert!(PrivateNetworkInput::new(
        &[b'x'; 33],
        b"test-passphrase",
        KnownSecurity::Wpa2Personal,
        0
    )
    .is_err());
    assert!(PrivateNetworkInput::new(b"a", &[b'x'; 64], KnownSecurity::Wpa2Personal, 0).is_err());
}
#[test]
fn unknown_stale_incompatible_and_partitioning_scans_never_displace_verified_body_route() {
    let mut p = KnownNetworkProfiles::new();
    let a = p.upsert(input(b"a", 0)).unwrap();
    p.upsert(input(b"b", 100)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    assert_eq!(
        policy
            .select(
                &p,
                20_000,
                &[scan(b"unknown", -1, 20_000), scan(b"a", -20, 0)],
                None
            )
            .unwrap(),
        KnownSelection::Wait
    );
    let mut incompatible = scan(b"a", -20, 20_001);
    incompatible.security = KnownSecurity::Wpa3Personal;
    assert_eq!(
        policy.select(&p, 20_001, &[incompatible], None).unwrap(),
        KnownSelection::Wait
    );
    let line = CurrentKnownLine {
        key: a,
        since_ms: 0,
        observed_ms: 90_000,
        healthy: true,
        verified_body_peer: true,
    };
    assert_eq!(
        policy
            .select(&p, 90_000, &[scan(b"b", -1, 90_000)], Some(line))
            .unwrap(),
        KnownSelection::Keep(a)
    );
    assert_eq!(
        policy.select(&p, 89_999, &[], None),
        Err(KnownNetworkRefusal::Clock)
    );
}
#[test]
fn minimum_dwell_margin_three_distinct_observations_and_pending_outcome_fence() {
    let mut p = KnownNetworkProfiles::new();
    let a = p.upsert(input(b"a", 0)).unwrap();
    let b = p.upsert(input(b"b", 1)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    let line = |now| {
        Some(CurrentKnownLine {
            key: a,
            since_ms: 0,
            observed_ms: now,
            healthy: true,
            verified_body_peer: false,
        })
    };
    assert_eq!(
        policy
            .select(
                &p,
                1_000,
                &[scan(b"a", -60, 1_000), scan(b"b", -20, 1_000)],
                line(1_000)
            )
            .unwrap(),
        KnownSelection::Keep(a)
    );
    assert_eq!(
        policy
            .select(
                &p,
                60_000,
                &[scan(b"a", -60, 60_000), scan(b"b", -55, 60_000)],
                line(60_000)
            )
            .unwrap(),
        KnownSelection::Keep(a)
    );
    for now in [61_000, 61_000, 62_000] {
        assert_eq!(
            policy
                .select(
                    &p,
                    now,
                    &[scan(b"a", -60, now), scan(b"b", -40, now)],
                    line(now)
                )
                .unwrap(),
            KnownSelection::Keep(a)
        );
    }
    assert_eq!(
        policy
            .select(
                &p,
                63_000,
                &[scan(b"a", -60, 63_000), scan(b"b", -40, 63_000)],
                line(63_000)
            )
            .unwrap(),
        KnownSelection::Attempt(b)
    );
    assert_eq!(
        policy.select(&p, 64_000, &[], None).unwrap(),
        KnownSelection::AwaitOutcome(b)
    );
    assert_eq!(
        policy.outcome(a, 64_000, false),
        Err(KnownNetworkRefusal::Outcome)
    );
    policy.outcome(b, 64_000, true).unwrap();
    assert_eq!(
        policy.outcome(b, 64_000, true),
        Err(KnownNetworkRefusal::Outcome)
    );
}
#[test]
fn three_attempts_per_round_stay_exhausted_without_explicit_round_renewal() {
    let mut p = KnownNetworkProfiles::new();
    let a = p.upsert(input(b"a", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    for index in 0..3 {
        let now = index * KNOWN_FAILURE_COOLDOWN_MS;
        assert_eq!(
            policy
                .select(&p, now, &[scan(b"a", -50, now)], None)
                .unwrap(),
            KnownSelection::Attempt(a)
        );
        policy.outcome(a, now, false).unwrap();
        assert_eq!(
            policy
                .select(&p, now + 1, &[scan(b"a", -50, now + 1)], None)
                .unwrap(),
            if index == 2 {
                KnownSelection::Exhausted
            } else {
                KnownSelection::Wait
            }
        );
    }
    assert_eq!(
        policy
            .select(&p, 100_000, &[scan(b"a", -1, 100_000)], None)
            .unwrap(),
        KnownSelection::Exhausted
    );
    let updated = p.upsert(input(b"a", 1)).unwrap();
    assert_ne!(updated, a);
    assert_eq!(
        policy
            .select(&p, 100_001, &[scan(b"a", -50, 100_001)], None)
            .unwrap(),
        KnownSelection::Attempt(updated)
    );
}
#[test]
fn deterministic_slot_ties_last_success_and_scan_bounds() {
    let mut p = KnownNetworkProfiles::new();
    let a = p.upsert(input(b"a", 0)).unwrap();
    let b = p.upsert(input(b"b", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    assert_eq!(
        policy
            .select(&p, 0, &[scan(b"b", -50, 0), scan(b"a", -50, 0)], None)
            .unwrap(),
        KnownSelection::Attempt(a)
    );
    policy.outcome(a, 0, false).unwrap();
    assert_eq!(
        policy.select(&p, 1, &[scan(b"b", -50, 1)], None).unwrap(),
        KnownSelection::Attempt(b)
    );
    policy.outcome(b, 1, true).unwrap();
    assert_eq!(
        policy
            .select(
                &p,
                30_001,
                &[scan(b"a", -50, 30_001), scan(b"b", -50, 30_001)],
                None
            )
            .unwrap(),
        KnownSelection::Attempt(b)
    );
    let mut policy = KnownNetworkPolicy::new();
    assert_eq!(
        policy.select(&p, 1, &[scan(b"a", -50, 2)], None),
        Err(KnownNetworkRefusal::Bounds)
    );
    let scans: [KnownScan<'_>; 17] = core::array::from_fn(|_| scan(b"a", -50, 0));
    assert_eq!(
        policy.select(&p, 1, &scans, None),
        Err(KnownNetworkRefusal::Bounds)
    );
}

#[test]
fn deadline_overflow_retains_pending_attempt_until_explicit_matching_outcome() {
    let mut profiles = KnownNetworkProfiles::new();
    let key = profiles.upsert(input(b"a", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    assert_eq!(
        policy
            .select(&profiles, u64::MAX, &[scan(b"a", -50, u64::MAX)], None)
            .unwrap(),
        KnownSelection::Attempt(key)
    );
    assert_eq!(
        policy.outcome(key, u64::MAX, false),
        Err(KnownNetworkRefusal::Clock)
    );
    assert_eq!(
        policy.select(&profiles, u64::MAX, &[], None).unwrap(),
        KnownSelection::AwaitOutcome(key)
    );
}

#[test]
fn long_router_outage_renews_explicit_round_only_after_fresh_scan_and_root_retirement_confirmation()
{
    assert_eq!(
        KNOWN_MAXIMUM_ATTEMPTS_PER_GENERATION,
        KNOWN_MAXIMUM_ATTEMPTS_PER_ROUND
    );
    let mut profiles = KnownNetworkProfiles::new();
    let key = profiles.upsert(input(b"a", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    for now in [0, 30_000, 60_000] {
        assert_eq!(
            policy
                .select(&profiles, now, &[scan(b"a", -50, now)], None)
                .unwrap(),
            KnownSelection::Attempt(key)
        );
        assert!(policy
            .request_recovery_round(&profiles, key, now, &scan(b"a", -50, now))
            .is_err());
        policy.outcome(key, now, false).unwrap();
    }
    assert!(policy
        .request_recovery_round(&profiles, key, 89_999, &scan(b"a", -50, 89_999))
        .is_err());
    // Merely waiting for an hour, or supplying an old scan, does not reset state.
    let now = 3_600_000;
    assert_eq!(
        policy
            .select(&profiles, now, &[scan(b"a", -50, now)], None)
            .unwrap(),
        KnownSelection::Exhausted
    );
    assert!(policy
        .request_recovery_round(&profiles, key, now, &scan(b"a", -50, 60_000))
        .is_err());
    let old = policy
        .request_recovery_round(&profiles, key, now, &scan(b"a", -50, now))
        .unwrap();
    assert_eq!(old.profile(), key);
    assert_eq!(old.exhausted_round(), 1);
    assert_eq!(
        policy
            .select(&profiles, now, &[scan(b"a", -50, now)], None)
            .unwrap(),
        KnownSelection::Exhausted
    );
    let current = policy
        .request_recovery_round(&profiles, key, now + 1, &scan(b"a", -50, now + 1))
        .unwrap();
    assert_eq!(
        policy.renew_after_retirement(&profiles, old, now + 1),
        Err(KnownNetworkRefusal::Outcome)
    );
    // This call models the Root-only integration boundary AFTER actual owner,
    // mandatory evidence and AP restoration checks; this CPU test has no radio.
    assert_eq!(
        policy
            .renew_after_retirement(&profiles, current, now + 1)
            .unwrap(),
        2
    );
    assert_eq!(
        policy
            .select(&profiles, now + 2, &[scan(b"a", -50, now + 2)], None)
            .unwrap(),
        KnownSelection::Attempt(key)
    );
    policy.outcome(key, now + 2, true).unwrap();
    let line = CurrentKnownLine {
        key,
        since_ms: now + 2,
        observed_ms: now + 3,
        healthy: true,
        verified_body_peer: true,
    };
    assert_eq!(
        policy.select(&profiles, now + 3, &[], Some(line)).unwrap(),
        KnownSelection::Keep(key)
    );
}

#[test]
fn stale_renewal_scan_and_replaced_profile_cannot_reopen_round() {
    let mut profiles = KnownNetworkProfiles::new();
    let key = profiles.upsert(input(b"a", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    for now in [0, 30_000, 60_000] {
        policy
            .select(&profiles, now, &[scan(b"a", -50, now)], None)
            .unwrap();
        policy.outcome(key, now, false).unwrap();
    }
    let stale = policy
        .request_recovery_round(&profiles, key, 90_000, &scan(b"a", -50, 90_000))
        .unwrap();
    assert_eq!(
        policy.renew_after_retirement(&profiles, stale, 106_000),
        Err(KnownNetworkRefusal::Bounds)
    );
    let changed = policy
        .request_recovery_round(&profiles, key, 106_000, &scan(b"a", -50, 106_000))
        .unwrap();
    profiles.upsert(input(b"a", 1)).unwrap();
    assert_eq!(
        policy.renew_after_retirement(&profiles, changed, 106_000),
        Err(KnownNetworkRefusal::StaleProfile)
    );
}

fn unknown(ssid: &[u8], rssi: i16, now: u64) -> KnownScanObservation<'_> {
    KnownScanObservation {
        ssid,
        security: ObservedNetworkSecurity::Unknown,
        rssi_dbm: rssi,
        observed_ms: now,
    }
}
#[test]
fn unknown_suite_reserves_only_exact_configured_backend_supported_auth_attempts() {
    let mut profiles = KnownNetworkProfiles::new();
    let wpa3 = profiles
        .upsert(
            PrivateNetworkInput::new(b"sae", b"protected-test", KnownSecurity::Wpa3Personal, 10)
                .unwrap(),
        )
        .unwrap();
    let wpa2 = profiles.upsert(input(b"psk", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    let none = KnownBackendAuth {
        wpa2_personal: false,
        wpa3_personal: false,
    };
    assert_eq!(
        policy
            .select_observed(
                &profiles,
                0,
                &[unknown(b"psk", -20, 0), unknown(b"sae", -10, 0)],
                None,
                none
            )
            .unwrap(),
        KnownSelection::Wait
    );
    let backend = KnownBackendAuth {
        wpa2_personal: true,
        wpa3_personal: false,
    };
    assert_eq!(
        policy
            .select_observed(
                &profiles,
                0,
                &[unknown(b"psk", -20, 0), unknown(b"sae", -10, 0)],
                None,
                backend
            )
            .unwrap(),
        KnownSelection::Attempt(wpa2)
    );
    assert!(matches!(
        unknown(b"psk", -20, 0).security,
        ObservedNetworkSecurity::Unknown
    ));
    policy.outcome(wpa2, 0, false).unwrap();
    // Explicit unknown still cannot weaken the higher-priority SAE profile to
    // WPA2; only its own supported configured authentication can be attempted.
    assert_eq!(
        policy
            .select_observed(&profiles, 1, &[unknown(b"sae", -10, 1)], None, backend)
            .unwrap(),
        KnownSelection::Wait
    );
    assert_eq!(
        policy
            .select_observed(
                &profiles,
                1,
                &[unknown(b"sae", -10, 1)],
                None,
                KnownBackendAuth {
                    wpa2_personal: false,
                    wpa3_personal: true
                }
            )
            .unwrap(),
        KnownSelection::Attempt(wpa3)
    );
    policy.outcome(wpa3, 1, false).unwrap();
    let mut unsupported = unknown(b"psk", -10, 30_000);
    unsupported.security = ObservedNetworkSecurity::Unsupported;
    assert_eq!(
        policy
            .select_observed(&profiles, 30_000, &[unsupported], None, backend)
            .unwrap(),
        KnownSelection::Wait
    );
    let mut incompatible = unknown(b"psk", -10, 30_000);
    incompatible.security = ObservedNetworkSecurity::Known(KnownSecurity::Wpa3Personal);
    assert_eq!(
        policy
            .select_observed(&profiles, 30_000, &[incompatible], None, backend)
            .unwrap(),
        KnownSelection::Wait
    );
}
#[test]
fn unknown_scan_never_supplies_body_route_success_or_reconnect_budget() {
    let mut profiles = KnownNetworkProfiles::new();
    let key = profiles.upsert(input(b"a", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    let backend = KnownBackendAuth {
        wpa2_personal: true,
        wpa3_personal: false,
    };
    for now in [0, 30_000, 60_000] {
        assert_eq!(
            policy
                .select_observed(&profiles, now, &[unknown(b"a", -10, now)], None, backend)
                .unwrap(),
            KnownSelection::Attempt(key)
        );
        assert_eq!(
            policy
                .select_observed(&profiles, now, &[unknown(b"a", -10, now)], None, backend)
                .unwrap(),
            KnownSelection::AwaitOutcome(key)
        );
        policy.outcome(key, now, false).unwrap();
    }
    assert_eq!(
        policy
            .select_observed(
                &profiles,
                90_000,
                &[unknown(b"a", -1, 90_000)],
                None,
                backend
            )
            .unwrap(),
        KnownSelection::Exhausted
    );
    // Even strongest possible scan cannot create a Keep/healthy peer result.
    // Only the independently supplied current-route observation can do that.
    let line = CurrentKnownLine {
        key,
        since_ms: 0,
        observed_ms: 90_000,
        healthy: true,
        verified_body_peer: true,
    };
    assert_eq!(
        policy
            .select_observed(
                &profiles,
                90_000,
                &[unknown(b"a", -1, 90_000)],
                Some(line),
                backend
            )
            .unwrap(),
        KnownSelection::Keep(key)
    );
    let mut other = KnownNetworkPolicy::new();
    assert_eq!(
        other
            .select_observed(
                &profiles,
                20_000,
                &[unknown(b"a", -1, 0), unknown(b"outside", -1, 20_000)],
                None,
                backend
            )
            .unwrap(),
        KnownSelection::Wait
    );
}
