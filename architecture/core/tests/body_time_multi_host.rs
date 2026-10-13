use conduit_core::{
    BodyClockCorrelation, BodyTimeQuality, BodyTimeRefusal, BodyTimeRelation, BodyTimeRequirement,
    BodyTimeTolerance, BodyTimeTracker, BootId, ClockProvenance, HostId, MonotonicClockIdentity,
    MonotonicDuration, MonotonicInstant, PeerClockExchange, PeerClockPolicy, TemporalScale,
};
use conduit_kernel::causal_evidence::{
    CausalEdge, CausalEvidence, CausalRelationship, EvidenceIdentity,
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

fn peer_correlation() -> BodyClockCorrelation {
    BodyClockCorrelation::new(
        "body/fixture".into(),
        TemporalScale::Milliseconds,
        1,
        sample("peer", "boot/peer", 8_000),
        10_000,
        0,
        100,
        2,
        10_000,
        ClockProvenance::External {
            provider_id: "fixture/source".into(),
            admission_reference: "fixture/admission".into(),
            policy_id: "fixture/policy".into(),
        },
    )
    .unwrap()
}

fn policy(local_boot: &str, sequence: u64) -> PeerClockPolicy {
    PeerClockPolicy {
        peer_host: HostId::from("peer"),
        peer_boot: BootId::from("boot/peer"),
        body_basis: "body/fixture".into(),
        minimum_generation: 1,
        maximum_round_trip: MonotonicDuration::new(20, TemporalScale::Milliseconds),
        maximum_local_rate_error_ppm: 100,
        correlation_horizon: MonotonicDuration::new(10_000, TemporalScale::Milliseconds),
        admission_id: format!("fixture/{local_boot}/{sequence}"),
        policy_id: "fixture/policy".into(),
        membership_revision: 7,
        observation_sequence: sequence,
    }
}

fn exchange(
    boot: &str,
    local_send: u64,
    peer_receive: u64,
    peer_send: u64,
    local_receive: u64,
) -> PeerClockExchange {
    PeerClockExchange {
        local_send: sample("local", boot, local_send),
        peer_receive: sample("peer", "boot/peer", peer_receive),
        peer_send: sample("peer", "boot/peer", peer_send),
        local_receive: sample("local", boot, local_receive),
    }
}

#[test]
fn deterministic_two_host_body_time_lifecycle() {
    let peer = peer_correlation();
    let first_exchange = exchange("boot/local/1", 1_000, 8_005, 8_006, 1_010);
    let first = first_exchange
        .derive_correlation(&peer, &policy("boot/local/1", 1), 2)
        .unwrap();
    assert_ne!(first.local_anchor().ticks(), peer.local_anchor().ticks());
    assert_eq!(first.body_basis(), peer.body_basis());
    assert_eq!(first.generation(), 2);
    assert!(matches!(
        first.project(first.local_anchor()).unwrap().provenance,
        ClockProvenance::Peer {
            membership_revision: 7,
            observation_sequence: 1,
            ..
        }
    ));

    let requirement = BodyTimeRequirement::new(
        "body/fixture".into(),
        BodyTimeTolerance::new(30, TemporalScale::Milliseconds),
        MonotonicDuration::new(100, TemporalScale::Milliseconds),
    )
    .unwrap();
    assert!(matches!(
        requirement.assess(&first, &sample("local", "boot/local/1", 1_010)),
        BodyTimeQuality::Ready { .. }
    ));
    let mut tracker = BodyTimeTracker::new(first, &sample("local", "boot/local/1", 1_010)).unwrap();
    let near = tracker.last().clone();
    let peer_near = peer.project(&sample("peer", "boot/peer", 8_010)).unwrap();
    assert_eq!(
        near.physical_relation(&peer_near),
        Ok(BodyTimeRelation::Indeterminate)
    );

    let deadline = sample("local", "boot/local/1", 1_010)
        .deadline_after(MonotonicDuration::new(100, TemporalScale::Milliseconds))
        .unwrap();
    let wall_before = 50_000_i64;
    for wall_after in [wall_before + 10_000, wall_before - 10_000] {
        assert_ne!(wall_after, wall_before);
        assert_eq!(
            deadline.remaining_at(&sample("local", "boot/local/1", 1_060)),
            Ok(Some(MonotonicDuration::new(
                50,
                TemporalScale::Milliseconds
            )))
        );
        assert_eq!(
            deadline.remaining_at(&sample("local", "boot/local/1", 1_110)),
            Ok(None)
        );
    }

    let during_partition = tracker
        .observe(&sample("local", "boot/local/1", 6_000))
        .unwrap()
        .clone();
    assert!(
        during_partition.latest_ticks - during_partition.center_ticks
            > near.latest_ticks - near.center_ticks
    );
    assert!(during_partition.center_ticks > near.center_ticks);
    assert!(matches!(
        BodyTimeRequirement::new(
            "body/fixture".into(),
            BodyTimeTolerance::new(15, TemporalScale::Milliseconds),
            MonotonicDuration::new(100, TemporalScale::Milliseconds),
        )
        .unwrap()
        .assess(
            tracker.correlation(),
            &sample("local", "boot/local/1", 6_000)
        ),
        BodyTimeQuality::Unsupported {
            reason: BodyTimeRefusal::InsufficientQuality
        }
    ));

    let bad_policy = PeerClockPolicy {
        peer_boot: BootId::from("boot/forged"),
        ..policy("boot/local/1", 2)
    };
    let rejoin_exchange = exchange("boot/local/1", 6_010, 13_016, 13_017, 6_020);
    let local_elapsed =
        rejoin_exchange.local_receive.ticks() - first_exchange.local_receive.ticks();
    let peer_elapsed = rejoin_exchange.peer_send.ticks() - first_exchange.peer_send.ticks();
    assert_eq!(local_elapsed, 5_010);
    assert_eq!(peer_elapsed, 5_011);
    assert_ne!(local_elapsed, peer_elapsed);
    assert_eq!(
        rejoin_exchange.derive_correlation(&peer, &bad_policy, 3),
        Err(BodyTimeRefusal::UnadmittedPeer)
    );
    assert_eq!(
        exchange("boot/local/1", 6_010, 13_016, 13_017, 6_100).derive_correlation(
            &peer,
            &policy("boot/local/1", 2),
            3
        ),
        Err(BodyTimeRefusal::ExcessiveRoundTrip)
    );
    let rejoined = rejoin_exchange
        .derive_correlation(&peer, &policy("boot/local/1", 2), 3)
        .unwrap();
    let before_rejoin = tracker.last().clone();
    let after_rejoin = tracker
        .reconcile(rejoined, &sample("local", "boot/local/1", 6_020))
        .unwrap()
        .clone();
    assert_eq!(after_rejoin.generation, 3);
    assert!(after_rejoin.center_ticks >= before_rejoin.center_ticks);
    assert_eq!(before_rejoin.generation, 2);
    assert_eq!(before_rejoin.local_sample.ticks(), 6_000);

    let send = peer.project(&sample("peer", "boot/peer", 13_017)).unwrap();
    let receive = after_rejoin.clone();
    let send_evidence = EvidenceIdentity {
        sign: 1,
        execution: 1,
        host_session: 1,
    };
    let receive_evidence = EvidenceIdentity {
        sign: 2,
        execution: 1,
        host_session: 2,
    };
    let mut causal = CausalEvidence::<8>::default();
    causal
        .record(CausalEdge {
            effect: receive_evidence,
            relationship: CausalRelationship::CausedBy,
            cause: send_evidence,
        })
        .unwrap();
    assert_eq!(
        send.physical_relation(&receive),
        Ok(BodyTimeRelation::Indeterminate)
    );
    assert_eq!(
        causal.cause_of(receive_evidence, CausalRelationship::CausedBy),
        Ok(send_evidence)
    );
    assert_eq!(send.local_sample.clock().host_id().as_str(), "peer");
    assert_eq!(receive.local_sample.clock().host_id().as_str(), "local");
    assert_ne!(send.local_sample.clock(), receive.local_sample.clock());

    assert_eq!(
        tracker.observe(&sample("local", "boot/local/2", 10)),
        Err(BodyTimeRefusal::DifferentClock)
    );
    let reboot_exchange = exchange("boot/local/2", 10, 13_020, 13_021, 20);
    let rebooted = reboot_exchange
        .derive_correlation(&peer, &policy("boot/local/2", 3), 4)
        .unwrap();
    assert_eq!(
        rebooted.project(&sample("local", "boot/local/1", 6_030)),
        Err(BodyTimeRefusal::DifferentClock)
    );
    let reboot_estimate = rebooted
        .project(&sample("local", "boot/local/2", 20))
        .unwrap();
    assert_eq!(
        reboot_estimate.local_sample.clock().boot_id().as_str(),
        "boot/local/2"
    );
    assert!(reboot_estimate.center_ticks > after_rejoin.center_ticks);
}
