use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, AmbientAdmissionProof, Body,
    BodyMembership, CandidateInventory, CandidateObservation, DiscoveryProofId, PartReturnProof,
};
use conduit_core::{
    BootId, CheckedPlotId, HostAdvertisement, HostId, HostProfileId, LinkBindingId,
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
        bases: vec![],
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn admitted() -> (AdmissionManager, BodyMembership, SigningKey) {
    let body = Body::born(
        SourceDocumentId::from("source/continuity"),
        CheckedPlotId::from("checked/continuity"),
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
    let returned_credential = manager
        .complete_return(
            &mut membership,
            &returned,
            &return_proof(&challenge, &key),
            2_101,
            SignId::from("sign/host-returned"),
        )
        .unwrap();
    assert_eq!(returned_credential.body_id, membership.body_id);
    assert_eq!(returned_credential.part_id, part_id);
    assert_eq!(
        returned_credential.host_id,
        HostId::from("host/durable-browser")
    );
    assert_eq!(returned_credential.boot_id, BootId::from("boot/fresh"));
    assert_ne!(
        returned_credential.credential_id,
        manager.receipts[0].credential.credential_id
    );
    assert_eq!(
        manager.receipts.last().unwrap().credential,
        returned_credential
    );
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

#[test]
fn current_member_message_requires_retained_key_exact_membership_and_signed_transcript() {
    let (manager, mut membership, key) = admitted();
    let retained_manager = manager.clone();
    let retained_membership = membership.clone();
    let part = membership.parts[0].part_id.clone();
    let host = HostId::from("host/durable-browser");
    let boot = BootId::from("boot/old");
    let transcript = [91; 32];
    let signature = key.sign(&transcript).to_bytes();
    let verified = manager
        .verify_admitted_message(&membership, &part, &host, &boot, &transcript, &signature)
        .unwrap();
    assert_eq!(verified.body_id(), &membership.body_id);
    assert_eq!(verified.part_id(), &part);
    assert_eq!(verified.host_id(), &host);
    assert_eq!(verified.boot_id(), &boot);
    assert_eq!(verified.membership_revision(), membership.revision);
    assert_eq!(verified.transcript(), &transcript);
    for (claimed_host, claimed_boot, message, signed) in [
        (
            HostId::from("host/foreign"),
            boot.clone(),
            transcript,
            signature,
        ),
        (
            host.clone(),
            BootId::from("boot/stale"),
            transcript,
            signature,
        ),
        (host.clone(), boot.clone(), [92; 32], signature),
        (
            host.clone(),
            boot.clone(),
            transcript,
            SigningKey::from_bytes(&[93; 32])
                .sign(&transcript)
                .to_bytes(),
        ),
    ] {
        assert!(manager
            .verify_admitted_message(
                &membership,
                &part,
                &claimed_host,
                &claimed_boot,
                &message,
                &signed
            )
            .is_err());
    }
    let mut foreign = membership.clone();
    foreign.body_id = Body::born(
        "source/foreign".into(),
        "checked/foreign".into(),
        1,
        "sign/foreign".into(),
    )
    .unwrap()
    .body_id;
    assert!(manager
        .verify_admitted_message(&foreign, &part, &host, &boot, &transcript, &signature)
        .is_err());
    assert_eq!(manager, retained_manager);
    assert_eq!(membership, retained_membership);
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &part,
            &boot,
            "sign/message-offline".into(),
        )
        .unwrap();
    assert!(manager
        .verify_admitted_message(&membership, &part, &host, &boot, &transcript, &signature)
        .is_err());
    // A matching caller-held key cannot establish custody in a fresh manager.
    let empty = AdmissionManager::new(membership.body_id.clone()).unwrap();
    assert!(empty
        .verify_admitted_message(
            &retained_membership,
            &part,
            &host,
            &boot,
            &transcript,
            &signature
        )
        .is_err());
}

#[test]
fn reboot_challenge_retirement_preserves_continuity_and_consumed_receipts() {
    let (mut manager, membership, key) = admitted();
    let invitation = manager
        .issue_spawn_invitation(
            conduit_body::SpawnInvitationSecret::from_csprng_bytes([81; 32]).unwrap(),
            [82; 32],
            100,
            200,
        )
        .unwrap();
    let receipts = manager.receipts.clone();
    manager.retire_unfinished_challenges();
    assert_eq!(manager.receipts, receipts);
    let current = membership.parts[0].current.as_ref().unwrap();
    let transcript = [83; 32];
    manager
        .verify_admitted_message(
            &membership,
            &membership.parts[0].part_id,
            &current.host_id,
            &current.boot_id,
            &transcript,
            &key.sign(&transcript).to_bytes(),
        )
        .unwrap();
    let mut next = membership.clone();
    let host = advertisement("boot/old", 1);
    let proof = conduit_body::SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &host.host_id,
            &host.boot_id,
            host.offer_generation,
        )),
    };
    assert_eq!(
        manager.complete_spawn(
            &mut next,
            &host,
            &proof,
            101,
            AdmissionSigns {
                part_admitted: "sign/retired-part".into(),
                host_attached: "sign/retired-host".into(),
                candidate_admitted: "sign/retired-candidate".into(),
            }
        ),
        Err(AdmissionRefusal::UnknownInvitation)
    );
    assert_eq!(next, membership);
}

#[test]
fn retained_custody_survives_detachment_and_restart_without_restoring_presence() {
    let (mut manager, mut membership, key) = admitted();
    let credential = manager.receipts.last().unwrap().credential.clone();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &credential.part_id,
            &credential.boot_id,
            "sign/custody-offline".into(),
        )
        .unwrap();
    manager.retire_unfinished_challenges();
    // Actual durable manager/membership serialization preserves keys and completed
    // credentials while presence remains absent; no synthetic observe_present.
    let bytes = serde_json::to_vec(&(&manager, &membership)).unwrap();
    let (manager, membership): (AdmissionManager, BodyMembership) =
        serde_json::from_slice(&bytes).unwrap();
    let transcript = [81; 32];
    let signature = key.sign(&transcript).to_bytes();
    assert!(manager
        .verify_admitted_message(
            &membership,
            &credential.part_id,
            &credential.host_id,
            &credential.boot_id,
            &transcript,
            &signature,
        )
        .is_err());
    let verified = manager
        .verify_retained_custody_message(&membership, &credential, &transcript, &signature)
        .unwrap();
    assert_eq!(verified.credential(), &credential);
    assert_eq!(verified.body_id(), &membership.body_id);
    assert_eq!(verified.part_id(), &credential.part_id);
    assert_eq!(verified.host_id(), &credential.host_id);
    assert_eq!(verified.boot_id(), &credential.boot_id);
    assert_eq!(verified.membership_revision(), membership.revision);
    assert_eq!(verified.transcript(), &transcript);
    assert!(membership.parts[0].current.is_none());
    assert_eq!(serde_json::to_vec(&(&manager, &membership)).unwrap(), bytes);
}

#[test]
fn retained_custody_refuses_foreign_forged_missing_and_revoked_credentials() {
    let (manager, mut membership, key) = admitted();
    let credential = manager.receipts.last().unwrap().credential.clone();
    let transcript = [82; 32];
    let signature = key.sign(&transcript).to_bytes();
    for mutation in 0..8 {
        let mut changed = credential.clone();
        let mut message = transcript;
        let mut signed = signature;
        match mutation {
            0 => {
                changed.body_id = Body::born(
                    "foreign/source".into(),
                    "foreign/checked".into(),
                    9,
                    "foreign/sign".into(),
                )
                .unwrap()
                .body_id
            }
            1 => changed.host_id = "foreign/host".into(),
            2 => changed.boot_id = "foreign/boot".into(),
            3 => {
                changed.credential_id =
                    serde_json::from_value(serde_json::json!("foreign/credential")).unwrap()
            }
            4 => changed.issued_at_millis += 1,
            5 => message[0] ^= 1,
            6 => signed[0] ^= 1,
            7 => signed = SigningKey::from_bytes(&[83; 32]).sign(&message).to_bytes(),
            _ => unreachable!(),
        }
        assert!(
            manager
                .verify_retained_custody_message(&membership, &changed, &message, &signed)
                .is_err(),
            "mutation {mutation}"
        );
    }
    let mut missing = manager.clone();
    missing.receipts.clear();
    assert!(missing
        .verify_retained_custody_message(&membership, &credential, &transcript, &signature)
        .is_err());
    let fresh = AdmissionManager::new(membership.body_id.clone()).unwrap();
    assert!(fresh
        .verify_retained_custody_message(&membership, &credential, &transcript, &signature)
        .is_err());
    let mut receipts_without_key = fresh;
    receipts_without_key.receipts = manager.receipts.clone();
    assert!(
        receipts_without_key
            .verify_retained_custody_message(&membership, &credential, &transcript, &signature)
            .is_err(),
        "matching public receipts must not replace the private continuity key"
    );
    membership
        .revoke(
            &membership.body_id.clone(),
            membership.revision,
            &credential.part_id,
            "sign/custody-revoked".into(),
        )
        .unwrap();
    assert!(manager
        .verify_retained_custody_message(&membership, &credential, &transcript, &signature)
        .is_err());
}

#[test]
fn retained_custody_fences_old_boot_after_canonical_return_and_later_detachment() {
    let (mut manager, mut membership, key) = admitted();
    let old = manager.receipts.last().unwrap().credential.clone();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &old.part_id,
            &old.boot_id,
            "sign/custody-lost".into(),
        )
        .unwrap();
    let returned = advertisement("boot/custody-next", 1);
    let challenge = manager
        .begin_return(&membership, &old.part_id, &returned, [84; 32], 2100, 3100)
        .unwrap();
    let latest = manager
        .complete_return(
            &mut membership,
            &returned,
            &return_proof(&challenge, &key),
            2101,
            "sign/custody-return".into(),
        )
        .unwrap();
    let restored: AdmissionManager =
        serde_json::from_slice(&serde_json::to_vec(&manager).unwrap()).unwrap();
    assert_eq!(
        restored.receipts, manager.receipts,
        "completed receipt append order survives durable replay"
    );
    let manager = restored;
    let transcript = [85; 32];
    let signature = key.sign(&transcript).to_bytes();
    assert_eq!(
        manager.latest_completed_credential(&old.part_id),
        Some(&latest)
    );
    assert_eq!(
        manager.verify_retained_custody_message(&membership, &old, &transcript, &signature),
        Err(AdmissionRefusal::StaleBoot)
    );
    manager
        .verify_retained_custody_message(&membership, &latest, &transcript, &signature)
        .unwrap();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &latest.part_id,
            &latest.boot_id,
            "sign/custody-next-lost".into(),
        )
        .unwrap();
    assert_eq!(
        manager.verify_retained_custody_message(&membership, &old, &transcript, &signature),
        Err(AdmissionRefusal::StaleBoot)
    );
    manager
        .verify_retained_custody_message(&membership, &latest, &transcript, &signature)
        .unwrap();
    // Even a signed credential is not enough if live membership has a different
    // incarnation than the completed receipt retained by this manager.
    let mut inconsistent = membership.clone();
    inconsistent
        .observe_present(
            &inconsistent.body_id.clone(),
            inconsistent.revision,
            &latest.part_id,
            conduit_body::AuthenticatedHostObservation {
                host_id: latest.host_id.clone(),
                boot_id: "boot/not-in-completed-receipt".into(),
                offer_generation: OfferGeneration(2),
                proof_id: conduit_body::MembershipProofId::bind("proof/custody-other-presence")
                    .unwrap(),
                sequence: inconsistent.revision.0 + 1,
            },
            "sign/custody-other-presence".into(),
        )
        .unwrap();
    assert_eq!(
        manager.verify_retained_custody_message(&inconsistent, &latest, &transcript, &signature),
        Err(AdmissionRefusal::StaleBoot)
    );
}
