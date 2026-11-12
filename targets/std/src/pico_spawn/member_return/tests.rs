use super::*;
use conduit_body::SpawnInvitationSecret;
use conduit_core::OfferGeneration;
fn id<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_value(serde_json::json!(text)).unwrap()
}
fn fixture() -> (
    PicoMemberReturnBasis,
    HostAdvertisement,
    SpawnInvitationSecret,
) {
    let key = SpawnInvitationSecret::from_csprng_bytes([9; 32]).unwrap();
    let mut ad = crate::StdHost::new().advertisement().clone();
    ad.bases.clear();
    ad.resources.clear();
    ad.capabilities.clear();
    ad.planner_capabilities.clear();
    let credential: MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id":"credential/old","body_id":"body/house","part_id":"part/mcu",
        "host_id":ad.host_id,"boot_id":"boot/old","issued_at_millis":1}))
    .unwrap();
    let owner_key = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let basis = PicoMemberReturnBasis {
        credential,
        expected_profile: ad.profile.clone(),
        member_verifying_key: key.verifying_key(),
        owner: FederationPeer {
            host_id: "host/owner".into(),
            boot_id: "boot/owner".into(),
            offer_generation: OfferGeneration(1),
            verifying_key: owner_key.verifying_key(),
        },
    };
    (basis, ad, key)
}
#[test]
fn presentation_never_substitutes_part_profile_or_retained_credential() {
    let (basis, ad, key) = fixture();
    let projection = conduit_body::HostAdvertisementImageProjection {
        authority_grants: vec![],
        context: ad,
        capability_images: vec![],
    };
    let signature = key
        .sign(
            &conduit_body::member_return_image_transcript(&basis.credential, &projection).unwrap(),
        )
        .to_vec();
    let mut presentation = MemberReturnPresentation {
        signature,
        kind: "Advertise".into(),
        schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
        credential: basis.credential.clone(),
        projection,
    };
    basis.check_presentation(&presentation).unwrap();
    presentation.credential.part_id = id("part/other");
    assert!(basis.check_presentation(&presentation).is_err());
    presentation.credential = basis.credential.clone();
    presentation.projection.context.profile = "profile/other".into();
    assert!(basis.check_presentation(&presentation).is_err());
    let mut wrong = basis;
    wrong.owner.verifying_key = [0; 32];
    assert!(wrong.validate().is_err());
}
#[test]
fn canonical_proof_wire_and_signature_are_exactly_scoped() {
    let (basis, ad, key) = fixture();
    let challenge = PartReturnChallenge {
        admission_id: id("admission/return"),
        body_id: id("body/house"),
        part_id: id("part/mcu"),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
        nonce: [3; 32],
        issued_at_millis: 1,
        expires_at_millis: 100,
    };
    let mut proof = PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    };
    basis.check_proof(&ad, &challenge, &proof).unwrap();
    let view = conduit_body::BorrowedPartReturnProof {
        admission_id: proof.admission_id.as_str(),
        body_id: proof.body_id.as_str(),
        part_id: proof.part_id.as_str(),
        host_id: proof.host_id.as_str(),
        boot_id: proof.boot_id.as_str(),
        nonce: proof.nonce,
        signature: &proof.signature,
    };
    let document: MemberReturnProofDocument =
        serde_json::from_slice(&serde_json::to_vec(&view).unwrap()).unwrap();
    assert!(document.decode().unwrap() == proof);
    proof.signature[0] ^= 1;
    assert!(basis.check_proof(&ad, &challenge, &proof).is_err());
    proof.signature[0] ^= 1;
    proof.nonce = [4; 32];
    assert!(basis.check_proof(&ad, &challenge, &proof).is_err());
    proof.nonce = challenge.nonce;
    let mut other = challenge;
    other.offer_generation.0 += 1;
    assert!(basis.check_proof(&ad, &other, &proof).is_err());
}

#[path = "basis_advancement_tests.rs"]
mod basis_advancement;
