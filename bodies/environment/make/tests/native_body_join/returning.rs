//! Actual simulated-Store return custody, not an SDK or network proof.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    mut owner: conduit_body::SealedBodyLifecycleSession,
    root_key: &[u8; 32],
    host: &c::HostAdvertisement,
    observer: &c::HostAdvertisement,
    receipt: &native_joined_host_wire::JoinedHostReceipt,
    secret: &conduit_body::SpawnInvitationSecret,
    signer: &SigningKey,
    media: &native_evidence_region::BodyRegion,
) -> conduit_body::SealedBodyLifecycleSession {
    // Real Store return uses canonical continuity, not retained receipt custody.
    owner
        .observe_host_lost(
            &observer.host_id,
            &observer.boot_id,
            &host.host_id,
            &host.boot_id,
        )
        .unwrap();
    native_restart::checkpoint(&mut owner, root_key).unwrap();
    let mut fresh = observer.clone();
    fresh.boot_id = "boot/observer-returned".into();
    let begin_json = serde_json::json!({"schema":native_body_join::return_wire::CHALLENGE_REQUEST,"body_id":owner.evidence().body_id,"part_id":receipt.credential.part_id,"host_advertisement":fresh});
    let begin_bytes = serde_json::to_vec(&begin_json).unwrap();
    assert!(begin_bytes.len() <= 1024);
    let begin = match native_body_join::decode_command(&begin_bytes).unwrap() {
        native_body_join::Command::ReturnChallenge(begin) => begin,
        _ => panic!("return challenge dispatch changed"),
    };
    assert_eq!(begin.advertisement.boot_id, fresh.boot_id);
    for field in ["bases", "resources", "capabilities", "planner_capabilities"] {
        let mut bad = begin_json.clone();
        bad["host_advertisement"][field] = serde_json::json!([{"rich":"refuse"}]);
        assert!(native_body_join::decode_command(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let draft = native_body_join::returning::begin(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &begin,
        [121; 32],
        1000,
    )
    .unwrap();
    let challenge_hash = draft.transcript();
    let challenge = draft.signed(signer.sign(&challenge_hash).to_bytes());
    assert!(serde_json::to_vec(&challenge).unwrap().len() <= 2048);
    let generation = native_restart::generation();
    let retry = native_body_join::returning::begin(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &begin,
        [122; 32],
        1001,
    )
    .unwrap();
    assert_eq!(challenge_hash, retry.transcript());
    assert_eq!(native_restart::generation(), generation);
    // Actual journal reopen preserves the exact pending challenge on response loss.
    let mut boxed = Some(Box::new(owner));
    let parked =
        native_restart::park_box(&mut boxed, root_key, &host.host_id, &host.boot_id).unwrap();
    owner =
        *native_restart::restore_parked(parked, &host.host_id, &host.boot_id, root_key).unwrap();
    let reopened_generation = native_restart::generation();
    let reopened = native_body_join::returning::begin(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &begin,
        [123; 32],
        1002,
    )
    .unwrap();
    assert_eq!(challenge_hash, reopened.transcript());
    assert_eq!(native_restart::generation(), reopened_generation);
    // Exact-scope expiry retires only this old challenge, then checkpoints a
    // fresh nonce. The prior proof cannot complete against the replacement.
    let expired_proof = conduit_body::PartReturnProof {
        admission_id: challenge.challenge.admission_id.clone(),
        body_id: challenge.challenge.body_id.clone(),
        part_id: challenge.challenge.part_id.clone(),
        host_id: fresh.host_id.clone(),
        boot_id: fresh.boot_id.clone(),
        nonce: challenge.challenge.nonce,
        signature: secret.sign(&challenge.challenge.signing_transcript()),
    };
    let draft = native_body_join::returning::begin(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &begin,
        [125; 32],
        31_001,
    )
    .unwrap();
    let renewed_hash = draft.transcript();
    let challenge = draft.signed(signer.sign(&renewed_hash).to_bytes());
    assert_ne!(renewed_hash, challenge_hash);
    assert!(native_restart::generation() > reopened_generation);
    let replaced_generation = native_restart::generation();
    let retry = native_body_join::returning::begin(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &begin,
        [126; 32],
        31_002,
    )
    .unwrap();
    assert_eq!(retry.transcript(), renewed_hash);
    assert_eq!(native_restart::generation(), replaced_generation);
    let expired = native_body_join::return_wire::Complete {
        advertisement: fresh.clone(),
        proof: expired_proof,
    };
    assert!(native_body_join::returning::complete(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &expired,
        31_003
    )
    .is_err());
    let complete_json = serde_json::json!({"schema":native_body_join::return_wire::REQUEST,"body_id":challenge.challenge.body_id,"part_id":challenge.challenge.part_id,"admission_id":challenge.challenge.admission_id,"host_advertisement":fresh,"nonce":challenge.challenge.nonce,"signature":secret.sign(&challenge.challenge.signing_transcript()).iter().map(|b|format!("{b:02x}")).collect::<String>()});
    let complete_bytes = serde_json::to_vec(&complete_json).unwrap();
    assert!(complete_bytes.len() <= 1024);
    let complete = match native_body_join::decode_command(&complete_bytes).unwrap() {
        native_body_join::Command::Return(complete) => complete,
        _ => panic!("return completion dispatch changed"),
    };
    assert_eq!(complete.proof.nonce, challenge.challenge.nonce);
    let prior = owner.evidence().clone();
    let mut bad = native_body_join::return_wire::complete(&complete_bytes).unwrap();
    bad.proof.signature[0] ^= 1;
    assert!(native_body_join::returning::complete(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &bad,
        31_004
    )
    .is_err());
    assert_eq!(owner.evidence(), &prior);
    let returned = native_body_join::returning::complete(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &complete,
        31_005,
    )
    .unwrap();
    let returned_hash = returned.transcript();
    let returned = returned.signed(signer.sign(&returned_hash).to_bytes());
    let mut query = recovery::Request {
        schema: recovery::REQUEST.into(),
        body_id: owner.evidence().body_id.clone(),
        host_id: fresh.host_id.clone(),
        boot_id: fresh.boot_id.clone(),
        nonce: challenge.challenge.nonce,
        signature: vec![],
    };
    query.signature = secret.sign(&query.transcript()).to_vec();
    returned.verify(root_key, &query).unwrap();
    assert_eq!(returned.credential.part_id, receipt.credential.part_id);
    assert_eq!(returned.credential.boot_id, fresh.boot_id);
    assert!(native_body_join::returning::complete(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &complete,
        31_006
    )
    .is_err());
    native_restart::with_admissions(|manager| {
        recovery::verify_current(
            manager,
            &owner.evidence().membership,
            &query,
            &host.host_id,
            &host.boot_id,
            root_key,
        )
        .map(|_| ())
    })
    .unwrap();
    // A failed completion checkpoint returns no signable witness and fences
    // publication; the retained pre-completion bank remains recoverable.
    owner
        .observe_host_lost(&fresh.host_id, &fresh.boot_id, &host.host_id, &host.boot_id)
        .unwrap();
    native_restart::checkpoint(&mut owner, root_key).unwrap();
    let mut another = begin_json.clone();
    another["host_advertisement"]["boot_id"] = "boot/observer-next".into();
    let another =
        native_body_join::return_wire::begin(&serde_json::to_vec(&another).unwrap()).unwrap();
    let draft = native_body_join::returning::begin(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &another,
        [124; 32],
        32_000,
    )
    .unwrap();
    let hash = draft.transcript();
    let challenge = draft.signed(signer.sign(&hash).to_bytes());
    let mut last = complete_json.clone();
    last["host_advertisement"]["boot_id"] = "boot/observer-next".into();
    last["admission_id"] = serde_json::to_value(&challenge.challenge.admission_id).unwrap();
    last["nonce"] = serde_json::json!(challenge.challenge.nonce);
    last["signature"] = serde_json::json!(secret
        .sign(&challenge.challenge.signing_transcript())
        .to_vec());
    let last =
        native_body_join::return_wire::complete(&serde_json::to_vec(&last).unwrap()).unwrap();
    media.0.lock().unwrap().1 = false;
    assert!(native_body_join::returning::complete(
        &mut owner,
        root_key,
        &host.host_id,
        &host.boot_id,
        &last,
        32_001
    )
    .is_err());
    assert!(!native_restart::publication_ready());
    owner
}
