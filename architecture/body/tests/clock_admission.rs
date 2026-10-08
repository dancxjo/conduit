use conduit_body::{
    AuthenticatedHostObservation, Body, BodyClockAdmissionRefusal, BodyClockPeerAdmission,
    BodyClockSourcePolicy, BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{
    BodyClockCorrelation, BodyTimeRefusal, BodyTimeTracker, BootId, ClockProvenance, HostId,
    MonotonicClockIdentity, MonotonicDuration, MonotonicInstant, OfferGeneration,
    PeerClockExchange, SignId, TemporalScale,
};

fn sample(host: &str, boot: &str, ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            host.into(),
            boot.into(),
            "steady".into(),
            TemporalScale::Milliseconds,
            1,
            1,
        )
        .unwrap(),
    )
    .unwrap()
}

fn membership() -> (BodyMembership, PartId) {
    let body = Body::born(
        "source/test".into(),
        "checked/test".into(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let body_id = body.body_id;
    let part = PartId::bind(&body_id, "peer", 1).unwrap();
    let proof = MembershipProofId::bind("proof/peer").unwrap();
    let mut membership = BodyMembership::new(body_id.clone()).unwrap();
    membership
        .admit(
            &body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from("sign/admitted"),
        )
        .unwrap();
    membership
        .observe_present(
            &body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: HostId::from("host/peer"),
                boot_id: BootId::from("boot/peer"),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 4,
            },
            SignId::from("sign/present"),
        )
        .unwrap();
    (membership, part)
}

fn peer_correlation(body_basis: &str, body_ticks: u64) -> BodyClockCorrelation {
    BodyClockCorrelation::new(
        body_basis.into(),
        TemporalScale::Milliseconds,
        1,
        sample("host/peer", "boot/peer", 8_000),
        body_ticks,
        0,
        100,
        2,
        100,
        ClockProvenance::External {
            provider_id: "provider/local".into(),
            admission_reference: "proof/seed".into(),
            policy_id: "policy/seed".into(),
        },
    )
    .unwrap()
}

fn exchange() -> PeerClockExchange {
    PeerClockExchange {
        local_send: sample("host/local", "boot/local", 1_000),
        peer_receive: sample("host/peer", "boot/peer", 8_005),
        peer_send: sample("host/peer", "boot/peer", 8_006),
        local_receive: sample("host/local", "boot/local", 1_010),
    }
}

fn admission(
    membership: &BodyMembership,
    part: &PartId,
) -> Result<BodyClockPeerAdmission, BodyClockAdmissionRefusal> {
    let policy = source_policy(membership, part);
    BodyClockPeerAdmission::prepare(
        membership,
        &membership.body_id,
        part,
        &HostId::from("host/peer"),
        &BootId::from("boot/peer"),
        &policy,
    )
}

fn source_policy(membership: &BodyMembership, part: &PartId) -> BodyClockSourcePolicy {
    BodyClockSourcePolicy {
        body_id: membership.body_id.clone(),
        part_id: part.clone(),
        membership_proof: membership.parts[0]
            .current
            .as_ref()
            .unwrap()
            .proof_id
            .clone(),
        policy_id: "policy/clock".into(),
        minimum_generation: 1,
        maximum_round_trip: MonotonicDuration::new(20, TemporalScale::Milliseconds),
        maximum_local_rate_error_ppm: 100,
        correlation_horizon: MonotonicDuration::new(100, TemporalScale::Milliseconds),
    }
}

#[test]
fn membership_without_clock_source_authority_is_insufficient() {
    let (membership, part) = membership();
    let mut policy = source_policy(&membership, &part);
    policy.membership_proof = MembershipProofId::bind("proof/other").unwrap();
    assert_eq!(
        BodyClockPeerAdmission::prepare(
            &membership,
            &membership.body_id,
            &part,
            &HostId::from("host/peer"),
            &BootId::from("boot/peer"),
            &policy,
        ),
        Err(BodyClockAdmissionRefusal::UnauthorizedClockSource)
    );
}

#[test]
fn revoked_clock_source_policy_refuses_later_exchange() {
    let (membership, part) = membership();
    let admission = admission(&membership, &part).unwrap();
    let mut current_policy = source_policy(&membership, &part);
    current_policy.policy_id = "policy/revoked".into();
    assert_eq!(
        admission.derive(
            &membership,
            &current_policy,
            &HostId::from("host/peer"),
            &BootId::from("boot/peer"),
            &exchange(),
            &peer_correlation(membership.body_id.as_str(), 10_000),
            2,
        ),
        Err(BodyClockAdmissionRefusal::StalePolicy)
    );
}

#[test]
fn current_membership_and_transport_identity_admit_peer_clock_evidence() {
    let (membership, part) = membership();
    let admission = admission(&membership, &part).unwrap();
    let policy = source_policy(&membership, &part);
    let correlation = admission
        .derive(
            &membership,
            &policy,
            &HostId::from("host/peer"),
            &BootId::from("boot/peer"),
            &exchange(),
            &peer_correlation(membership.body_id.as_str(), 10_000),
            2,
        )
        .unwrap();
    assert_eq!(correlation.body_basis(), membership.body_id.as_str());
    assert_eq!(correlation.generation(), 2);
    let proof_reference = membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .proof_id
        .as_str();
    let estimate = correlation
        .project(&sample("host/local", "boot/local", 1_010))
        .unwrap();
    assert_eq!(
        estimate.provenance,
        ClockProvenance::Peer {
            host_id: HostId::from("host/peer"),
            boot_id: BootId::from("boot/peer"),
            admission_reference: proof_reference.into(),
            policy_id: "policy/clock".into(),
            membership_revision: membership.revision.0,
            observation_sequence: 4,
        }
    );
}

#[test]
fn stale_membership_and_wrong_transport_peer_refuse_replayed_clock_evidence() {
    let (mut membership, part) = membership();
    let admission = admission(&membership, &part).unwrap();
    let policy = source_policy(&membership, &part);
    assert_eq!(
        admission.derive(
            &membership,
            &policy,
            &HostId::from("host/other"),
            &BootId::from("boot/peer"),
            &exchange(),
            &peer_correlation(membership.body_id.as_str(), 10_000),
            2,
        ),
        Err(BodyClockAdmissionRefusal::WrongTransportPeer)
    );
    let body_id = membership.body_id.clone();
    membership
        .observe_offline(
            &body_id,
            membership.revision,
            &part,
            &BootId::from("boot/peer"),
            SignId::from("sign/offline"),
        )
        .unwrap();
    assert_eq!(
        admission.derive(
            &membership,
            &policy,
            &HostId::from("host/peer"),
            &BootId::from("boot/peer"),
            &exchange(),
            &peer_correlation(body_id.as_str(), 10_000),
            2,
        ),
        Err(BodyClockAdmissionRefusal::StaleMembership)
    );
}

#[test]
fn admitted_rejoin_updates_one_tracker_and_quarantines_conflicting_peer_time() {
    let (membership, part) = membership();
    let admission = admission(&membership, &part).unwrap();
    let policy = source_policy(&membership, &part);
    let local_start = sample("host/local", "boot/local", 1_000);
    let initial = BodyClockCorrelation::new(
        membership.body_id.as_str().into(),
        TemporalScale::Milliseconds,
        1,
        local_start.clone(),
        10_000,
        0,
        100,
        2,
        100,
        ClockProvenance::External {
            provider_id: "provider/seed".into(),
            admission_reference: "proof/seed".into(),
            policy_id: "policy/seed".into(),
        },
    )
    .unwrap();
    let mut tracker = BodyTimeTracker::new(initial, &local_start).unwrap();
    let before = tracker.last().clone();
    let updated = admission
        .reconcile(
            &membership,
            &policy,
            &HostId::from("host/peer"),
            &BootId::from("boot/peer"),
            &exchange(),
            &peer_correlation(membership.body_id.as_str(), 10_000),
            2,
            &mut tracker,
        )
        .unwrap();
    assert!(updated.center_ticks >= before.center_ticks);
    assert_eq!(updated.generation, 2);
    let accepted = tracker.last().clone();
    assert_eq!(
        admission.reconcile(
            &membership,
            &policy,
            &HostId::from("host/peer"),
            &BootId::from("boot/peer"),
            &exchange(),
            &peer_correlation(membership.body_id.as_str(), 20_000),
            3,
            &mut tracker,
        ),
        Err(BodyClockAdmissionRefusal::Clock(
            BodyTimeRefusal::ConflictingEvidence
        ))
    );
    assert_eq!(tracker.last(), &accepted);
}
