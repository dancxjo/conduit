use conduit_body::{
    Body, BodyMembership, CandidateInventory, CandidateObservation, CandidateRefusal,
    CandidateState, DiscoveryProofId, IngressFailureKind, MembershipProofId, PartId,
    MAX_CANDIDATES, MAX_CANDIDATE_ADVERTISEMENT_BYTES, MAX_CANDIDATE_CAPABILITIES,
    MAX_CANDIDATE_TOTAL_BYTES, MAX_INGRESS_REFUSALS,
};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, CheckedFormId,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindContractRevision, KindId, LinkBindingId, OfferGeneration, SignId, SourceDocumentId,
    PROTOCOL_VERSION,
};

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source/candidates"),
        CheckedFormId::from("checked/candidates"),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap()
}

fn advertisement(host: &str, boot: &str, generation: u64) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("profile/untrusted-peer"),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn capability(index: usize) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("claimed/capability/{index}")),
        kind_id: KindId::from(format!("claimed/kind/{index}")),
        kind_contract_revision: KindContractRevision::from("claimed/revision"),
        inputs: Vec::new(),
        outputs: Vec::new(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("claimed/profile"),
            implementation_id: ImplementationId::from(format!("claimed/implementation/{index}")),
            artifact_id: ArtifactId::from("claimed/artifact"),
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: u16::MAX,
            max_queue_items: u16::MAX,
            max_queue_bytes: u32::MAX,
        },
    }
}

fn observation(
    host: &str,
    boot: &str,
    generation: u64,
    freshness: u64,
    proof: &str,
    sign: &str,
) -> CandidateObservation {
    CandidateObservation {
        advertisement: advertisement(host, boot, generation),
        friendly_label: "Friendly peer".into(),
        observed_binding_id: LinkBindingId::from("line/observed-only"),
        observation_sign_id: SignId::from(sign),
        proof_id: DiscoveryProofId::bind(proof).unwrap(),
        freshness_sequence: freshness,
        encoded_bytes: 512,
    }
}

#[test]
fn advertisement_enters_as_exact_inert_candidate_without_membership_or_authority() {
    let body = body();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let local_part = PartId::bind(&body.body_id, "local", 0).unwrap();
    membership
        .admit(
            &body.body_id,
            membership.revision,
            local_part,
            MembershipProofId::bind("birth").unwrap(),
            SignId::from("sign/local-admitted"),
        )
        .unwrap();
    let membership_before = membership.clone();
    let mut inventory = CandidateInventory::new(body.body_id.clone()).unwrap();
    let mut claimed = observation(
        "host/hostile",
        "boot/hostile-1",
        7,
        1,
        "proof/transport-a",
        "sign/discovered",
    );
    claimed.friendly_label = "This computer".into();
    claimed.advertisement.capabilities = (0..MAX_CANDIDATE_CAPABILITIES).map(capability).collect();
    let exact = claimed.clone();

    let candidate_id = inventory.observe(claimed).unwrap();

    assert_eq!(inventory.candidates.len(), 1);
    let candidate = &inventory.candidates[0];
    assert_eq!(candidate.candidate_id, candidate_id);
    assert_eq!(candidate.state, CandidateState::Discovered);
    assert_eq!(candidate.observation, exact);
    assert_eq!(candidate.observation.friendly_label, "This computer");
    assert_eq!(
        candidate.observation.advertisement.capabilities.len(),
        MAX_CANDIDATE_CAPABILITIES
    );
    assert_eq!(membership, membership_before);
    assert_eq!(membership.parts.len(), 1);
}

#[test]
fn candidate_lifecycle_is_explicit_and_never_implicitly_advances() {
    let body = body();
    let mut inventory = CandidateInventory::new(body.body_id).unwrap();
    let candidate = inventory
        .observe(observation(
            "host/a",
            "boot/a",
            1,
            1,
            "proof/a",
            "sign/discovered",
        ))
        .unwrap();
    assert_eq!(inventory.candidates[0].state, CandidateState::Discovered);

    inventory
        .transition(
            &candidate,
            CandidateState::RequestingAdmission,
            SignId::from("sign/requesting"),
        )
        .unwrap();
    assert_eq!(
        inventory.transition(
            &candidate,
            CandidateState::Discovered,
            SignId::from("sign/backwards")
        ),
        Err(CandidateRefusal::InvalidTransition)
    );
    inventory
        .transition(
            &candidate,
            CandidateState::Refused,
            SignId::from("sign/refused"),
        )
        .unwrap();
    assert_eq!(inventory.candidates[0].state, CandidateState::Refused);

    for (index, terminal) in [
        CandidateState::Lost,
        CandidateState::Expired,
        CandidateState::Admitted,
    ]
    .into_iter()
    .enumerate()
    {
        let next = inventory
            .observe(observation(
                format!("host/terminal/{index}").as_str(),
                format!("boot/terminal/{index}").as_str(),
                1,
                1,
                format!("proof/terminal/{index}").as_str(),
                format!("sign/terminal/{index}/discovered").as_str(),
            ))
            .unwrap();
        if terminal == CandidateState::Admitted {
            inventory
                .transition(
                    &next,
                    CandidateState::RequestingAdmission,
                    SignId::from(format!("sign/terminal/{index}/requesting")),
                )
                .unwrap();
        }
        inventory
            .transition(
                &next,
                terminal,
                SignId::from(format!("sign/terminal/{index}/done")),
            )
            .unwrap();
    }
    assert!(inventory
        .candidates
        .iter()
        .any(|candidate| candidate.state == CandidateState::Lost));
    assert!(inventory
        .candidates
        .iter()
        .any(|candidate| candidate.state == CandidateState::Expired));
    assert!(inventory
        .candidates
        .iter()
        .any(|candidate| candidate.state == CandidateState::Admitted));
}

#[test]
fn operator_may_refuse_a_discovered_candidate_without_starting_admission() {
    let body = body();
    let mut inventory = CandidateInventory::new(body.body_id).unwrap();
    let candidate = inventory
        .observe(observation(
            "host/refused",
            "boot/refused",
            1,
            1,
            "proof/refused",
            "sign/refused-discovered",
        ))
        .unwrap();

    inventory
        .transition(
            &candidate,
            CandidateState::Refused,
            SignId::from("sign/operator-refused"),
        )
        .unwrap();

    assert_eq!(inventory.candidates[0].state, CandidateState::Refused);
    assert_eq!(
        inventory.history.last().unwrap().sign_id.as_str(),
        "sign/operator-refused"
    );
}

#[test]
fn stale_duplicate_conflicting_proof_and_replayed_boot_refuse_without_mutation() {
    let body = body();
    let mut inventory = CandidateInventory::new(body.body_id).unwrap();
    let first = observation("host/a", "boot/a", 3, 3, "proof/a", "sign/first");
    inventory.observe(first.clone()).unwrap();
    let retained = inventory.clone();
    assert_eq!(
        inventory.observe(first),
        Err(CandidateRefusal::DuplicateObservation)
    );
    assert_eq!(inventory, retained);
    assert_eq!(
        inventory.observe(observation(
            "host/a",
            "boot/a",
            4,
            3,
            "proof/forged",
            "sign/conflict"
        )),
        Err(CandidateRefusal::ConflictingProof)
    );
    assert_eq!(inventory, retained);
    assert_eq!(
        inventory.observe(observation(
            "host/a",
            "boot/a",
            2,
            4,
            "proof/a",
            "sign/stale-generation"
        )),
        Err(CandidateRefusal::StaleOfferGeneration)
    );
    assert_eq!(inventory, retained);

    inventory
        .observe(observation(
            "host/a",
            "boot/b",
            1,
            4,
            "proof/b",
            "sign/new-boot",
        ))
        .unwrap();
    let fresh = inventory.clone();
    assert_eq!(
        inventory.observe(observation(
            "host/a",
            "boot/a",
            5,
            5,
            "proof/a",
            "sign/replayed-boot"
        )),
        Err(CandidateRefusal::StaleBoot)
    );
    assert_eq!(inventory, fresh);
}

#[test]
fn malformed_oversized_and_disconnected_ingress_are_explicit_and_inert() {
    let body = body();
    let mut inventory = CandidateInventory::new(body.body_id).unwrap();
    inventory
        .record_incomplete(
            IngressFailureKind::MalformedFraming,
            LinkBindingId::from("line/a"),
            SignId::from("sign/malformed"),
            19,
        )
        .unwrap();
    inventory
        .record_incomplete(
            IngressFailureKind::DisconnectedBeforeComplete,
            LinkBindingId::from("line/a"),
            SignId::from("sign/disconnected"),
            31,
        )
        .unwrap();
    let mut oversized = observation("host/a", "boot/a", 1, 1, "proof/a", "sign/oversized");
    oversized.encoded_bytes = MAX_CANDIDATE_ADVERTISEMENT_BYTES + 1;
    assert_eq!(
        inventory.observe(oversized),
        Err(CandidateRefusal::OversizedAdvertisement)
    );
    assert!(inventory.candidates.is_empty());
    assert_eq!(inventory.history.len(), 0);
    assert_eq!(inventory.ingress_failures.len(), 3);
    assert_eq!(
        inventory.ingress_failures[2].kind,
        IngressFailureKind::OversizedAdvertisement
    );
}

#[test]
fn candidate_item_byte_and_refusal_history_bounds_are_hard() {
    let initial_body = body();
    let mut inventory = CandidateInventory::new(initial_body.body_id).unwrap();
    let each = MAX_CANDIDATE_TOTAL_BYTES / MAX_CANDIDATES as u32;
    for index in 0..MAX_CANDIDATES {
        let mut next = observation(
            format!("host/{index}").as_str(),
            format!("boot/{index}").as_str(),
            1,
            1,
            format!("proof/{index}").as_str(),
            format!("sign/{index}").as_str(),
        );
        next.encoded_bytes = each;
        inventory.observe(next).unwrap();
    }
    assert_eq!(
        inventory.observe(observation(
            "host/overflow",
            "boot/overflow",
            1,
            1,
            "proof/overflow",
            "sign/overflow"
        )),
        Err(CandidateRefusal::CandidateCapacityExhausted)
    );

    let refusal_body = body();
    let mut failures = CandidateInventory::new(refusal_body.body_id).unwrap();
    for index in 0..MAX_INGRESS_REFUSALS {
        failures
            .record_incomplete(
                IngressFailureKind::MalformedFraming,
                LinkBindingId::from("line/refusal"),
                SignId::from(format!("sign/refusal/{index}")),
                index as u32,
            )
            .unwrap();
    }
    assert_eq!(
        failures.record_incomplete(
            IngressFailureKind::MalformedFraming,
            LinkBindingId::from("line/refusal"),
            SignId::from("sign/refusal/overflow"),
            0,
        ),
        Err(CandidateRefusal::RefusalHistoryCapacityExhausted)
    );

    let byte_body = body();
    let mut bytes = CandidateInventory::new(byte_body.body_id).unwrap();
    for index in 0..4 {
        let mut next = observation(
            format!("host/bytes/{index}").as_str(),
            format!("boot/bytes/{index}").as_str(),
            1,
            1,
            format!("proof/bytes/{index}").as_str(),
            format!("sign/bytes/{index}").as_str(),
        );
        next.encoded_bytes = MAX_CANDIDATE_ADVERTISEMENT_BYTES;
        bytes.observe(next).unwrap();
    }
    assert_eq!(
        bytes.observe(observation(
            "host/bytes/overflow",
            "boot/bytes/overflow",
            1,
            1,
            "proof/bytes/overflow",
            "sign/bytes/overflow"
        )),
        Err(CandidateRefusal::ByteCapacityExhausted)
    );
}

#[test]
fn candidate_event_history_is_finite() {
    let body = body();
    let mut inventory = CandidateInventory::new(body.body_id).unwrap();
    inventory
        .observe(observation(
            "host/history",
            "boot/history/0",
            1,
            0,
            "proof/history/0",
            "sign/history/0",
        ))
        .unwrap();
    for sequence in 1..conduit_body::MAX_CANDIDATE_HISTORY {
        inventory
            .observe(observation(
                "host/history",
                format!("boot/history/{sequence}").as_str(),
                1,
                sequence as u64,
                format!("proof/history/{sequence}").as_str(),
                format!("sign/history/{sequence}").as_str(),
            ))
            .unwrap();
    }
    let retained = inventory.clone();
    assert_eq!(
        inventory.observe(observation(
            "host/history",
            "boot/history/overflow",
            1,
            conduit_body::MAX_CANDIDATE_HISTORY as u64,
            "proof/history/overflow",
            "sign/history/overflow"
        )),
        Err(CandidateRefusal::HistoryCapacityExhausted)
    );
    assert_eq!(inventory, retained);
}

#[test]
fn wrong_protocol_and_oversized_claim_collections_do_not_enter_inventory() {
    let body = body();
    let mut inventory = CandidateInventory::new(body.body_id).unwrap();
    let mut wrong_protocol =
        observation("host/a", "boot/a", 1, 1, "proof/a", "sign/wrong-protocol");
    wrong_protocol.advertisement.protocol_version = PROTOCOL_VERSION + 1;
    assert_eq!(
        inventory.observe(wrong_protocol),
        Err(CandidateRefusal::WrongProtocol)
    );
    let mut too_many = observation("host/b", "boot/b", 1, 1, "proof/b", "sign/too-many");
    too_many.advertisement.capabilities =
        (0..=MAX_CANDIDATE_CAPABILITIES).map(capability).collect();
    assert_eq!(
        inventory.observe(too_many),
        Err(CandidateRefusal::MalformedAdvertisement)
    );
    assert!(inventory.candidates.is_empty());
}
