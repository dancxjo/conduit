//! Actual finite firmware codec over the unchanged canonical return documents.
use conduit_body::{
    BorrowedPartReturnChallenge, BorrowedPartReturnProof, MemberReturnAcknowledgement,
    MemberReturnProofDocument, MembershipCredential, PartReturnChallenge,
};
use conduit_core::{HostAdvertisement, OfferGeneration};
#[test]
fn embedded_codec_preserves_canonical_return_challenge_proof_and_acknowledgement() {
    let prior:MembershipCredential=serde_json::from_str(r#"{"credential_id":"credential/old","body_id":"body/house","part_id":"part/1","host_id":"host/mcu","boot_id":"boot/old","issued_at_millis":1}"#).unwrap();
    let ad = HostAdvertisement {
        declared_name: None,
        protocol_version: 1,
        host_id: prior.host_id.clone(),
        boot_id: "boot/new".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/checked".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let challenge = PartReturnChallenge {
        admission_id: serde_json::from_str("\"admission/1\"").unwrap(),
        body_id: prior.body_id.clone(),
        part_id: prior.part_id.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
        nonce: [3; 32],
        issued_at_millis: 1,
        expires_at_millis: 2,
    };
    let bytes = serde_json::to_vec(&challenge).unwrap();
    let (view, used) =
        serde_json_core::from_slice::<BorrowedPartReturnChallenge<'_>>(&bytes).unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(view.signing_transcript(), challenge.signing_transcript());
    assert!(view.matches_member(
        prior.body_id.as_str(),
        prior.part_id.as_str(),
        ad.host_id.as_str(),
        ad.boot_id.as_str(),
        1
    ));
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes([9; 32]).unwrap();
    let signature = key.sign(&view.signing_transcript());
    let proof = BorrowedPartReturnProof {
        admission_id: view.admission_id,
        body_id: view.body_id,
        part_id: view.part_id,
        host_id: view.host_id,
        boot_id: view.boot_id,
        nonce: view.nonce,
        signature: &signature,
    };
    let mut frame = [0; 4096];
    let n = serde_json_core::to_slice(&proof, &mut frame).unwrap();
    let decoded: MemberReturnProofDocument = serde_json::from_slice(&frame[..n]).unwrap();
    assert_eq!(decoded.decode().unwrap().signature, signature);
    let mut current = prior.clone();
    current.boot_id = ad.boot_id.clone();
    let ack = MemberReturnAcknowledgement::new(current, ad.clone());
    let bytes = serde_json::to_vec(&ack).unwrap();
    assert!(bytes.len() <= 4096);
    let (decoded, used) =
        serde_json_core::from_slice::<MemberReturnAcknowledgement>(&bytes).unwrap();
    assert_eq!(used, bytes.len());
    decoded.validate_for(&prior, &ad).unwrap();
}
