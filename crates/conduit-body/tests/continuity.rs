use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, AmbientAdmissionProof, Body,
    BodyMembership, CandidateInventory, CandidateObservation, DiscoveryProofId, PartReturnProof,
};
use conduit_core::{
    BootId, CheckedFormId, HostAdvertisement, HostId, HostProfileId, LinkBindingId,
    OfferGeneration, PlanId, SignId, SourceDocumentId, PROTOCOL_VERSION,
};
use ed25519_dalek::{Signer, SigningKey};

fn advertisement(boot: &str, generation: u64) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/durable-browser"),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("profile/browser"),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn admitted() -> (AdmissionManager, BodyMembership, SigningKey) {
    let body = Body::born(
        SourceDocumentId::from("source/continuity"),
        CheckedFormId::from("checked/continuity"),
        1,
        SignId::from("sign/continuity-born"),
    )
    .unwrap();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let first = advertisement("boot/old", 1);
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: first,
            friendly_label: "browser".into(),
            observed_binding_id: LinkBindingId::from("line/continuity"),
            observation_sign_id: SignId::from("sign/continuity-observed"),
            proof_id: DiscoveryProofId::bind("proof/continuity-discovery").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    let key = SigningKey::from_bytes(&[61; 32]);
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate,
            key.verifying_key().to_bytes(),
            [62; 32],
            1_000,
            2_000,
            SignId::from("sign/continuity-requested"),
        )
        .unwrap();
    let proof = AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    };
    let mut membership = BodyMembership::new(body.body_id).unwrap();
    manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            1_001,
            AdmissionSigns {
                part_admitted: SignId::from("sign/continuity-part"),
                host_attached: SignId::from("sign/continuity-host"),
                candidate_admitted: SignId::from("sign/continuity-candidate"),
            },
        )
        .unwrap();
    (manager, membership, key)
}

fn return_proof(
    challenge: &conduit_body::PartReturnChallenge,
    key: &SigningKey,
) -> PartReturnProof {
    PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    }
}

#[test]
fn offline_part_returns_under_same_identity_only_with_fresh_signed_boot() {
    let (mut manager, mut membership, key) = admitted();
    let part_id = membership.parts[0].part_id.clone();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &part_id,
            &BootId::from("boot/old"),
            SignId::from("sign/host-lost"),
        )
        .unwrap();
    assert!(!membership.parts[0].is_present());
    let active_plan = PlanId::from("plan/pinned-to-old-boot");
    let returned = advertisement("boot/fresh", 1);
    let challenge = manager
        .begin_return(&membership, &part_id, &returned, [63; 32], 2_100, 3_100)
        .unwrap();
    manager
        .complete_return(
            &mut membership,
            &returned,
            &return_proof(&challenge, &key),
            2_101,
            SignId::from("sign/host-returned"),
        )
        .unwrap();
    assert_eq!(membership.parts[0].part_id, part_id);
    assert_eq!(
        membership.parts[0].current.as_ref().unwrap().boot_id,
        BootId::from("boot/fresh")
    );
    assert_eq!(active_plan, PlanId::from("plan/pinned-to-old-boot"));
}

#[test]
fn stale_boot_wrong_key_and_offer_churn_cannot_revive_current_truth() {
    for failure in [
        AdmissionRefusal::StaleBoot,
        AdmissionRefusal::StaleOfferGeneration,
        AdmissionRefusal::InvalidProof,
    ] {
        let (mut manager, mut membership, key) = admitted();
        let part_id = membership.parts[0].part_id.clone();
        membership
            .observe_offline(
                &membership.body_id.clone(),
                membership.revision,
                &part_id,
                &BootId::from("boot/old"),
                SignId::from(format!("sign/offline/{failure:?}")),
            )
            .unwrap();
        let returned = advertisement("boot/fresh", 2);
        let challenge = manager
            .begin_return(&membership, &part_id, &returned, [71; 32], 2_000, 3_000)
            .unwrap();
        let mut observed = returned.clone();
        let proof_key = if failure == AdmissionRefusal::InvalidProof {
            SigningKey::from_bytes(&[72; 32])
        } else {
            key.clone()
        };
        if failure == AdmissionRefusal::StaleBoot {
            observed.boot_id = BootId::from("boot/old");
        }
        if failure == AdmissionRefusal::StaleOfferGeneration {
            observed.offer_generation = OfferGeneration(3);
        }
        let retained = membership.clone();
        assert!(matches!(
            manager.complete_return(
                &mut membership,
                &observed,
                &return_proof(&challenge, &proof_key),
                2_001,
                SignId::from("sign/refused-return")
            ),
            Err(actual) if actual == failure
        ));
        assert_eq!(membership, retained);
    }
}

#[test]
fn revoked_part_returns_only_as_an_inert_candidate() {
    let (mut manager, mut membership, _) = admitted();
    let part_id = membership.parts[0].part_id.clone();
    membership
        .revoke(
            &membership.body_id.clone(),
            membership.revision,
            &part_id,
            SignId::from("sign/part-revoked"),
        )
        .unwrap();
    let returned = advertisement("boot/after-revocation", 1);
    assert_eq!(
        manager.begin_return(&membership, &part_id, &returned, [81; 32], 3_000, 4_000),
        Err(AdmissionRefusal::CandidateNotEligible)
    );

    let mut candidates = CandidateInventory::new(membership.body_id.clone()).unwrap();
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: returned,
            friendly_label: "returned but revoked".into(),
            observed_binding_id: LinkBindingId::from("line/returned-revoked"),
            observation_sign_id: SignId::from("sign/returned-revoked-observed"),
            proof_id: DiscoveryProofId::bind("proof/returned-revoked").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    assert_eq!(candidates.candidates[0].candidate_id, candidate);
    assert_eq!(
        candidates.candidates[0].state,
        conduit_body::CandidateState::Discovered
    );
    assert!(!membership.parts[0].is_present());
}
