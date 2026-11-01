use super::*;
const BACKEND: KnownBackendAuth = KnownBackendAuth {
    wpa2_personal: true,
    wpa3_personal: false,
};
fn exhausted(
    priority: i8,
) -> (
    KnownNetworkProfiles,
    KnownNetworkPolicy,
    KnownProfileKey,
    KnownProfileKey,
) {
    let mut profiles = KnownNetworkProfiles::new();
    let a = profiles.upsert(input(b"a", priority)).unwrap();
    let b = profiles.upsert(input(b"b", 0)).unwrap();
    let mut policy = KnownNetworkPolicy::new();
    for now in [0, 30_000, 60_000] {
        for key in [a, b] {
            assert_eq!(
                policy
                    .select_saved(&profiles, now, Some(key), BACKEND)
                    .unwrap(),
                KnownSelection::Attempt(key)
            );
            policy.outcome(key, now, false).unwrap();
        }
    }
    (profiles, policy, a, b)
}
fn observed(
    ssid: &[u8],
    rssi: i16,
    when: u64,
    security: ObservedNetworkSecurity,
) -> KnownScanObservation<'_> {
    KnownScanObservation {
        ssid,
        security,
        rssi_dbm: rssi,
        observed_ms: when,
    }
}
#[test]
fn observed_round_uses_normal_priority_rssi_and_stable_slot_ranking_without_renewing() {
    for (priority, first_rssi, second_rssi, expected_first) in [
        (0, -70, -30, false),
        (1, -70, -30, true),
        (0, -30, -30, true),
    ] {
        let (profiles, mut policy, a, b) = exhausted(priority);
        let scans = [
            observed(
                b"a",
                first_rssi,
                100_000,
                ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
            ),
            observed(
                b"b",
                second_rssi,
                100_000,
                ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
            ),
        ];
        let renewal = policy
            .request_observed_recovery_round(&profiles, 100_000, scans.into_iter(), BACKEND)
            .unwrap();
        let wanted = if expected_first { a } else { b };
        assert_eq!(renewal.profile(), wanted);
        assert_eq!(
            policy
                .select_saved(&profiles, 100_000, None, BACKEND)
                .unwrap(),
            KnownSelection::Exhausted
        );
        // This fixture models the existing Root-only confirmation, not a radio.
        assert_eq!(
            policy.renew_after_retirement(&profiles, renewal, 100_000),
            Ok(2)
        );
        assert_eq!(
            policy
                .select_saved(&profiles, 100_000, Some(wanted), BACKEND)
                .unwrap(),
            KnownSelection::Attempt(wanted)
        );
    }
}
#[test]
fn unknown_and_unsupported_suites_never_renew_even_with_a_stronger_saved_ssid() {
    let (profiles, mut policy, _, b) = exhausted(0);
    let unknown = observed(b"a", -1, 100_000, ObservedNetworkSecurity::Unknown);
    let unsupported = observed(b"a", -1, 100_000, ObservedNetworkSecurity::Unsupported);
    let wrong_suite = observed(
        b"a",
        -1,
        100_000,
        ObservedNetworkSecurity::Known(KnownSecurity::Wpa3Personal),
    );
    for s in [unknown, unsupported, wrong_suite] {
        assert!(policy
            .request_observed_recovery_round(&profiles, 100_000, [s].into_iter(), BACKEND)
            .is_err());
    }
    let compatible = observed(
        b"b",
        -90,
        100_000,
        ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
    );
    assert_eq!(
        policy
            .request_observed_recovery_round(
                &profiles,
                100_000,
                [unknown, compatible].into_iter(),
                BACKEND
            )
            .unwrap()
            .profile(),
        b
    );
}
#[test]
fn observed_round_rejects_unbounded_future_stale_and_pre_cooldown_evidence() {
    let (profiles, mut policy, a, _) = exhausted(0);
    for (now, observed_ms, rssi) in [
        (89_999, 89_999, -50),
        (100_000, 89_999, -50),
        (120_000, 100_000, -50),
        (100_000, 100_001, -50),
        (100_000, 100_000, 1),
    ] {
        let scan = observed(
            b"a",
            rssi,
            observed_ms,
            ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
        );
        assert!(policy
            .request_observed_recovery_round(&profiles, now, [scan].into_iter(), BACKEND)
            .is_err());
    }
    let scan = observed(
        b"a",
        -50,
        100_000,
        ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
    );
    assert!(policy
        .request_observed_recovery_round(&profiles, 100_000, [scan; 17].into_iter(), BACKEND)
        .is_err());
    assert_eq!(
        policy
            .request_observed_recovery_round(&profiles, 100_000, [scan].into_iter(), BACKEND)
            .unwrap()
            .profile(),
        a
    );
}
