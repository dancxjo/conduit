use conduit_core::{
    BodyClockCorrelation, BodyTimeRefusal, BootId, ClockProvenance, HostId, MonotonicClockIdentity,
    MonotonicDuration, MonotonicInstant, PeerClockExchange, PeerClockPolicy, TemporalScale,
};

fn sample(host: &str, boot: &str, ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            HostId::from(host),
            BootId::from(boot),
            "steady".into(),
            TemporalScale::Milliseconds,
            1,
            1,
        )
        .unwrap(),
    )
    .unwrap()
}

fn peer() -> BodyClockCorrelation {
    BodyClockCorrelation::new(
        "body/one".into(),
        TemporalScale::Milliseconds,
        1,
        sample("peer", "boot/peer", 8_000),
        10_000,
        0,
        100,
        2,
        100,
        ClockProvenance::External {
            provider_id: "provider/test".into(),
            admission_reference: "admitted/source/peer".into(),
            policy_id: "policy/test".into(),
        },
    )
    .unwrap()
}

fn exchange(local_receive: u64) -> PeerClockExchange {
    PeerClockExchange {
        local_send: sample("local", "boot/local", 1_000),
        peer_receive: sample("peer", "boot/peer", 8_005),
        peer_send: sample("peer", "boot/peer", 8_006),
        local_receive: sample("local", "boot/local", local_receive),
    }
}

fn policy() -> PeerClockPolicy {
    PeerClockPolicy {
        peer_host: HostId::from("peer"),
        peer_boot: BootId::from("boot/peer"),
        body_basis: "body/one".into(),
        minimum_generation: 1,
        maximum_round_trip: MonotonicDuration::new(20, TemporalScale::Milliseconds),
        maximum_local_rate_error_ppm: 100,
        correlation_horizon: MonotonicDuration::new(100, TemporalScale::Milliseconds),
        admission_id: "admitted/peer-exchange/1".into(),
        policy_id: "policy/exchange".into(),
        membership_revision: 3,
        observation_sequence: 4,
    }
}

#[test]
fn exchange_establishes_bounded_local_correlation_without_symmetric_delay_claim() {
    let correlation = exchange(1_010)
        .derive_correlation(&peer(), &policy(), 2)
        .unwrap();
    let estimate = correlation
        .project(&sample("local", "boot/local", 1_010))
        .unwrap();
    assert_eq!(estimate.generation, 2);
    assert_eq!(estimate.body_basis, "body/one");
    assert_eq!(estimate.local_sample.ticks(), 1_010);
    assert_eq!(
        estimate.provenance,
        ClockProvenance::Peer {
            host_id: HostId::from("peer"),
            boot_id: BootId::from("boot/peer"),
            admission_reference: "admitted/peer-exchange/1".into(),
            policy_id: "policy/exchange".into(),
            membership_revision: 3,
            observation_sequence: 4,
        }
    );
    assert!(estimate.earliest_ticks <= 10_006);
    assert!(estimate.latest_ticks >= 10_016);
}

#[test]
fn exchange_refuses_wrong_boot_and_excessive_round_trip() {
    let mut bad_policy = policy();
    bad_policy.peer_boot = BootId::from("boot/other");
    assert_eq!(
        exchange(1_010).derive_correlation(&peer(), &bad_policy, 2),
        Err(BodyTimeRefusal::UnadmittedPeer)
    );
    assert_eq!(
        exchange(1_100).derive_correlation(&peer(), &policy(), 2),
        Err(BodyTimeRefusal::ExcessiveRoundTrip)
    );
}
