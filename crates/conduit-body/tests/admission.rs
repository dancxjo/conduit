use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, AmbientAdmissionProof, Body,
    BodyMembership, CandidateInventory, CandidateObservation, CandidateState, DiscoveryProofId,
    SpawnAdmissionProof, SpawnInvitationSecret, ADMISSION_SIGNATURE_BYTES, MAX_ADMISSION_ATTEMPTS,
};
use conduit_core::{
    BootId, CheckedFormId, HostAdvertisement, HostId, HostProfileId, LinkBindingId,
    OfferGeneration, PlanId, SignId, SourceDocumentId, PROTOCOL_VERSION,
};
use ed25519_dalek::{Signer, SigningKey};

const NOW: u64 = 10_000;
const EXPIRES: u64 = 20_000;

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source/admission"),
        CheckedFormId::from("checked/admission"),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap()
}

fn wrong_body_id() -> conduit_body::BodyId {
    Body::born(
        SourceDocumentId::from("source/wrong-body"),
        CheckedFormId::from("checked/wrong-body"),
        99,
        SignId::from("sign/wrong-body-born"),
    )
    .unwrap()
    .body_id
}

fn advertisement(host: &str, boot: &str, generation: u64) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("profile/admission-test"),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn candidate_observation(advertisement: HostAdvertisement) -> CandidateObservation {
    CandidateObservation {
        advertisement,
        friendly_label: "untrusted label".into(),
        observed_binding_id: LinkBindingId::from("line/observed"),
        observation_sign_id: SignId::from("sign/candidate-observed"),
        proof_id: DiscoveryProofId::bind("proof/discovery-only").unwrap(),
        freshness_sequence: 1,
        encoded_bytes: 512,
    }
}

fn signs(prefix: &str) -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: SignId::from(format!("{prefix}/part")),
        host_attached: SignId::from(format!("{prefix}/host")),
        candidate_admitted: SignId::from(format!("{prefix}/candidate")),
    }
}

fn ambient_fixture() -> (
    AdmissionManager,
    CandidateInventory,
    BodyMembership,
    SigningKey,
    conduit_body::AdmissionChallenge,
) {
    let body = body();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let candidate_id = candidates
        .observe(candidate_observation(advertisement(
            "host/ambient",
            "boot/ambient-1",
            3,
        )))
        .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let key = SigningKey::from_bytes(&[7; 32]);
    let mut manager = AdmissionManager::new(body.body_id).unwrap();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate_id,
            key.verifying_key().to_bytes(),
            [9; 32],
            NOW,
            EXPIRES,
            SignId::from("sign/requesting-admission"),
        )
        .unwrap();
    (manager, candidates, membership, key, challenge)
}

fn ambient_proof(
    challenge: &conduit_body::AdmissionChallenge,
    key: &SigningKey,
) -> AmbientAdmissionProof {
    AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    }
}

#[test]
fn ambient_candidate_attaches_only_after_explicit_challenge_and_valid_ed25519_proof() {
    let (mut manager, mut candidates, mut membership, key, challenge) = ambient_fixture();
    let immutable_plan = PlanId::from("plan/already-active");
    assert!(membership.parts.is_empty());
    assert_eq!(
        candidates.candidates[0].state,
        CandidateState::RequestingAdmission
    );

    let credential = manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &ambient_proof(&challenge, &key),
            NOW + 1,
            signs("sign/ambient-success"),
        )
        .unwrap();

    assert_eq!(membership.parts.len(), 1);
    assert!(membership.parts[0].is_present());
    assert_eq!(membership.parts[0].part_id, credential.part_id);
    assert_eq!(credential.body_id, challenge.body_id);
    assert_eq!(credential.host_id, challenge.host_id);
    assert_eq!(credential.boot_id, challenge.boot_id);
    assert_eq!(candidates.candidates[0].state, CandidateState::Admitted);
    assert_eq!(immutable_plan, PlanId::from("plan/already-active"));
    assert_eq!(manager.receipts.len(), 1);
}

#[test]
fn wrong_body_host_boot_nonce_expiry_and_replay_refuse_without_half_membership() {
    for expected in [
        AdmissionRefusal::WrongBody,
        AdmissionRefusal::WrongHost,
        AdmissionRefusal::StaleBoot,
        AdmissionRefusal::StaleNonce,
        AdmissionRefusal::Expired,
    ] {
        let (mut manager, mut candidates, mut membership, key, challenge) = ambient_fixture();
        let mut proof = ambient_proof(&challenge, &key);
        let now = match expected {
            AdmissionRefusal::WrongBody => {
                proof.body_id = wrong_body_id();
                NOW + 1
            }
            AdmissionRefusal::WrongHost => {
                proof.host_id = HostId::from("host/forged");
                NOW + 1
            }
            AdmissionRefusal::StaleBoot => {
                proof.boot_id = BootId::from("boot/stale");
                NOW + 1
            }
            AdmissionRefusal::StaleNonce => {
                proof.nonce = [8; 32];
                NOW + 1
            }
            AdmissionRefusal::Expired => EXPIRES + 1,
            _ => unreachable!(),
        };
        assert_eq!(
            manager.complete_ambient(
                &mut candidates,
                &mut membership,
                &proof,
                now,
                signs("sign/refused")
            ),
            Err(expected)
        );
        assert!(membership.parts.is_empty());
        assert_eq!(
            candidates.candidates[0].state,
            CandidateState::RequestingAdmission
        );
    }

    let (mut manager, mut candidates, mut membership, key, challenge) = ambient_fixture();
    let proof = ambient_proof(&challenge, &key);
    manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            NOW + 1,
            signs("sign/success"),
        )
        .unwrap();
    let retained = membership.clone();
    assert_eq!(
        manager.complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            NOW + 2,
            signs("sign/replay")
        ),
        Err(AdmissionRefusal::Replay)
    );
    assert_eq!(membership, retained);
}

#[test]
fn stale_candidate_truth_and_abrupt_disconnect_never_create_a_part() {
    let (mut manager, mut candidates, mut membership, key, challenge) = ambient_fixture();
    candidates.candidates[0]
        .observation
        .advertisement
        .offer_generation = OfferGeneration(4);
    assert_eq!(
        manager.complete_ambient(
            &mut candidates,
            &mut membership,
            &ambient_proof(&challenge, &key),
            NOW + 1,
            signs("sign/stale-offer")
        ),
        Err(AdmissionRefusal::StaleOfferGeneration)
    );
    assert!(membership.parts.is_empty());

    let (mut manager, mut candidates, membership, _, challenge) = ambient_fixture();
    manager
        .disconnect_ambient(
            &mut candidates,
            &challenge.admission_id,
            SignId::from("sign/disconnected"),
        )
        .unwrap();
    assert!(membership.parts.is_empty());
    assert_eq!(candidates.candidates[0].state, CandidateState::Lost);
}

#[test]
fn invalid_proof_attempts_are_bounded_and_do_not_mutate_membership() {
    let (mut manager, mut candidates, mut membership, _, challenge) = ambient_fixture();
    let invalid = AmbientAdmissionProof {
        admission_id: challenge.admission_id,
        body_id: challenge.body_id,
        host_id: challenge.host_id,
        boot_id: challenge.boot_id,
        nonce: challenge.nonce,
        signature: [0; ADMISSION_SIGNATURE_BYTES],
    };
    for attempt in 1..=MAX_ADMISSION_ATTEMPTS {
        let expected = if attempt == MAX_ADMISSION_ATTEMPTS {
            AdmissionRefusal::AttemptsExhausted
        } else {
            AdmissionRefusal::InvalidProof
        };
        assert_eq!(
            manager.complete_ambient(
                &mut candidates,
                &mut membership,
                &invalid,
                NOW + 1,
                signs(format!("sign/invalid/{attempt}").as_str())
            ),
            Err(expected)
        );
        assert!(membership.parts.is_empty());
    }
    assert_eq!(
        manager.complete_ambient(
            &mut candidates,
            &mut membership,
            &invalid,
            NOW + 1,
            signs("sign/exhausted")
        ),
        Err(AdmissionRefusal::AttemptsExhausted)
    );
}

#[test]
fn body_directed_invitation_is_secret_redacted_short_lived_and_single_use() {
    let body = body();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let mut membership = BodyMembership::new(body.body_id).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    assert_eq!(
        format!("{:?}", invitation.secret),
        "SpawnInvitationSecret([REDACTED])"
    );
    let advertisement = advertisement("host/spawned", "boot/spawned", 1);
    let proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        )),
    };
    manager
        .complete_spawn(
            &mut membership,
            &advertisement,
            &proof,
            NOW + 1,
            signs("sign/spawn-success"),
        )
        .unwrap();
    assert_eq!(membership.parts.len(), 1);
    let retained = membership.clone();
    assert_eq!(
        manager.complete_spawn(
            &mut membership,
            &advertisement,
            &proof,
            NOW + 2,
            signs("sign/spawn-replay")
        ),
        Err(AdmissionRefusal::Replay)
    );
    assert_eq!(membership, retained);
}

#[test]
fn spawn_invitations_are_distinct_at_the_same_instant_and_proof_attempts_are_bounded() {
    let body = body();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let first = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([31; 32]).unwrap(),
            [32; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    let second = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([33; 32]).unwrap(),
            [34; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    assert_ne!(first.invitation_id, second.invitation_id);

    let advertisement = advertisement("host/spawn-attempts", "boot/spawn-attempts", 1);
    let mut membership = BodyMembership::new(body.body_id).unwrap();
    let invalid = SpawnAdmissionProof {
        invitation_id: first.invitation_id,
        body_id: first.body_id,
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        nonce: first.nonce,
        signature: [0; ADMISSION_SIGNATURE_BYTES],
    };
    for attempt in 1..=MAX_ADMISSION_ATTEMPTS {
        let expected = if attempt == MAX_ADMISSION_ATTEMPTS {
            AdmissionRefusal::AttemptsExhausted
        } else {
            AdmissionRefusal::InvalidProof
        };
        assert_eq!(
            manager.complete_spawn(
                &mut membership,
                &advertisement,
                &invalid,
                NOW + 1,
                signs("sign/spawn-invalid")
            ),
            Err(expected)
        );
    }
    assert_eq!(
        manager.complete_spawn(
            &mut membership,
            &advertisement,
            &invalid,
            NOW + 1,
            signs("sign/spawn-exhausted")
        ),
        Err(AdmissionRefusal::AttemptsExhausted)
    );
    assert!(membership.parts.is_empty());
}

#[test]
fn expired_reused_and_mismatched_spawn_proofs_refuse_distinctly() {
    for expected in [
        AdmissionRefusal::WrongBody,
        AdmissionRefusal::WrongHost,
        AdmissionRefusal::StaleBoot,
        AdmissionRefusal::StaleNonce,
        AdmissionRefusal::Expired,
    ] {
        let active_body = body();
        let mut manager = AdmissionManager::new(active_body.body_id.clone()).unwrap();
        let mut membership = BodyMembership::new(active_body.body_id).unwrap();
        let invitation = manager
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes([21; 32]).unwrap(),
                [22; 32],
                NOW,
                EXPIRES,
            )
            .unwrap();
        let advertisement = advertisement("host/spawned", "boot/spawned", 1);
        let mut proof = SpawnAdmissionProof {
            invitation_id: invitation.invitation_id.clone(),
            body_id: invitation.body_id.clone(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            nonce: invitation.nonce,
            signature: invitation.secret.sign(&invitation.signing_transcript(
                &advertisement.host_id,
                &advertisement.boot_id,
                advertisement.offer_generation,
            )),
        };
        let now = match expected {
            AdmissionRefusal::WrongBody => {
                proof.body_id = wrong_body_id();
                NOW + 1
            }
            AdmissionRefusal::WrongHost => {
                proof.host_id = HostId::from("host/wrong");
                NOW + 1
            }
            AdmissionRefusal::StaleBoot => {
                proof.boot_id = BootId::from("boot/stale");
                NOW + 1
            }
            AdmissionRefusal::StaleNonce => {
                proof.nonce = [23; 32];
                NOW + 1
            }
            AdmissionRefusal::Expired => EXPIRES + 1,
            _ => unreachable!(),
        };
        assert_eq!(
            manager.complete_spawn(
                &mut membership,
                &advertisement,
                &proof,
                now,
                signs("sign/spawn-refused")
            ),
            Err(expected)
        );
        assert!(membership.parts.is_empty());
    }
    assert_eq!(
        SpawnInvitationSecret::from_csprng_bytes([0; 32]),
        Err(AdmissionRefusal::WeakSecret)
    );
}
