//! Actual native membership-only path through simulated durable Root custody.
use super::*;
use conduit_body::{
    PortableMembershipAdmissionRequest as Request, SealedBodyLifecycleSession,
    MEMBERSHIP_ADMISSION_REQUEST_SCHEMA,
};
pub(super) fn prove(
    mut owner: SealedBodyLifecycleSession,
    root: &[u8; 32],
    host: &c::HostAdvertisement,
    signer: &SigningKey,
) -> (SealedBodyLifecycleSession, Request) {
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([151; 32]).unwrap();
    let bootstrap = native_body_join::issue(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        secret.clone(),
        [152; 32],
        100,
        1000,
    )
    .unwrap();
    let claim = bootstrap.invitation.claim;
    let mut request = Request {
        schema: MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: c::PROTOCOL_VERSION,
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_id: "host/initialized-worker".into(),
        boot_id: "boot/initialized-worker".into(),
        offer_generation: c::OfferGeneration(17),
        nonce: claim.nonce,
        signature: [0; 64],
    };
    request.signature = secret.sign(&claim.signing_transcript(
        &request.host_id,
        &request.boot_id,
        request.offer_generation,
    ));
    let bytes = serde_json::to_vec(&request).unwrap();
    assert!(bytes.len() <= 1024);
    match native_body_join::decode_command(&bytes).unwrap() {
        native_body_join::Command::Membership(decoded) => assert_eq!(decoded, request),
        _ => panic!("membership-only request selected another profile"),
    }
    let mut rich: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    rich["host_advertisement"] = serde_json::json!({"capabilities":[{"hidden":"offer"}]});
    assert!(native_body_join::decode_command(&serde_json::to_vec(&rich).unwrap()).is_err());
    assert!(native_body_join::decode_command(&vec![b' '; 1025]).is_err());
    let before = owner.evidence().clone();
    let prior_plan = owner.realization().cloned();
    let manager = native_restart::with_admissions(|m| Ok(serde_json::to_vec(m).unwrap())).unwrap();
    let mut wrong = request.clone();
    wrong.signature[0] ^= 1;
    assert!(native_body_join::membership::admit(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        &wrong,
        101,
    )
    .is_err());
    assert_eq!(owner.evidence(), &before);
    assert_eq!(owner.realization(), prior_plan.as_ref());
    native_restart::with_admissions(|m| {
        assert_eq!(serde_json::to_vec(m).unwrap(), manager);
        Ok(())
    })
    .unwrap();
    let generation = native_restart::generation().unwrap();
    let draft = native_body_join::membership::admit(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        &request,
        101,
    )
    .unwrap();
    assert!(native_restart::publication_ready());
    assert!(native_restart::generation().unwrap() > generation);
    assert_eq!(draft.root(), root);
    let signature = signer.sign(&draft.transcript()).to_bytes();
    let receipt = draft.signed(signature);
    receipt.verify(root, &request).unwrap();
    assert!(!receipt.current_offers_available && !receipt.plan_created && !receipt.play_created);
    assert!(receipt.membership_admitted);
    assert_eq!(owner.realization(), prior_plan.as_ref());
    assert_eq!(owner.evidence().body, before.body);
    let mut response = [0u8; 2048];
    let written = serde_json_core::to_slice(&receipt, &mut response).unwrap();
    conduit_body::PortableMembershipAdmissionReceipt::decode(&response[..written])
        .unwrap()
        .verify(root, &request)
        .unwrap();
    let encoded = serde_json::to_vec(&receipt).unwrap();
    assert!(encoded.len() <= 2048);
    conduit_body::PortableMembershipAdmissionReceipt::decode(&encoded)
        .unwrap()
        .verify(root, &request)
        .unwrap();
    let admitted = owner.evidence().clone();
    assert!(native_body_join::membership::admit(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        &request,
        102,
    )
    .is_err());
    assert_eq!(owner.evidence(), &admitted);
    let mut boxed = Some(Box::new(owner));
    let parked = native_restart::park_box(&mut boxed, root, &host.host_id, &host.boot_id).unwrap();
    owner = *native_restart::restore_parked(parked, &host.host_id, &host.boot_id, root).unwrap();
    assert_eq!(owner.evidence(), &admitted);
    native_restart::with_admissions(|manager| {
        let transcript = [153; 32];
        manager
            .verify_admitted_message(
                &owner.evidence().membership,
                &receipt.credential.part_id,
                &request.host_id,
                &request.boot_id,
                &transcript,
                &secret.sign(&transcript),
            )
            .map_err(|_| "membership-continuity")?;
        Ok(())
    })
    .unwrap();
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([154; 32]).unwrap();
    let claim = native_body_join::issue(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        secret.clone(),
        [155; 32],
        110,
        1000,
    )
    .unwrap()
    .invitation
    .claim;
    request.invitation_id = claim.invitation_id.clone();
    request.host_id = "host/pending-worker".into();
    request.boot_id = "boot/pending-worker".into();
    request.nonce = claim.nonce;
    request.signature = secret.sign(&claim.signing_transcript(
        &request.host_id,
        &request.boot_id,
        request.offer_generation,
    ));
    (owner, request)
}
