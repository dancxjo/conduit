use super::*;
use crate::SpawnInvitationSecret;
use alloc::vec;
use conduit_core::{OfferGeneration, PROTOCOL_VERSION};
fn fixture() -> (
    MembershipCredential,
    HostAdvertisementImageProjection,
    FederationPeer,
    SpawnInvitationSecret,
) {
    let retained: MembershipCredential = serde_json::from_str(
        r#"{
      "credential_id":"credential/old","body_id":"body/house","part_id":"part/mcu",
      "host_id":"host/mcu","boot_id":"boot/old","issued_at_millis":1}"#,
    )
    .unwrap();
    let context = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: retained.host_id.clone(),
        boot_id: "boot/current".into(),
        offer_generation: OfferGeneration(2),
        profile: "profile/current".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let projection = HostAdvertisementImageProjection {
        context,
        capability_images: (0..16).map(|i| [i; 32]).collect(),
    };
    let key = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let owner = FederationPeer {
        host_id: "host/owner".into(),
        boot_id: "boot/owner".into(),
        offer_generation: OfferGeneration(1),
        verifying_key: key.verifying_key(),
    };
    (retained, projection, owner, key)
}
#[test]
fn member_signature_binds_entire_context_ordered_images_and_retained_relationship() {
    let (retained, projection, _, key) = fixture();
    let transcript = member_return_image_transcript(&retained, &projection).unwrap();
    let presentation = MemberReturnImagePresentation {
        kind: "Advertise".into(),
        schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
        credential: retained.clone(),
        projection: projection.clone(),
        signature: key.sign(&transcript).into(),
    };
    presentation.verify(&key.verifying_key()).unwrap();
    let mut frame = [0; crate::MAX_PICO_ADMISSION_FRAME_BYTES];
    let used = serde_json_core::to_slice(&presentation, &mut frame).unwrap();
    let hosted: MemberReturnImagePresentation = serde_json::from_slice(&frame[..used]).unwrap();
    hosted.verify(&key.verifying_key()).unwrap();
    for field in [
        "boot",
        "profile",
        "generation",
        "image",
        "order",
        "part",
        "credential",
        "schema",
    ] {
        let mut changed = presentation.clone();
        match field {
            "boot" => changed.projection.context.boot_id = "boot/other".into(),
            "profile" => changed.projection.context.profile = "profile/other".into(),
            "generation" => changed.projection.context.offer_generation.0 += 1,
            "image" => changed.projection.capability_images[0][0] ^= 1,
            "order" => changed.projection.capability_images.swap(0, 1),
            "part" => changed.credential.part_id = serde_json::from_str("\"part/other\"").unwrap(),
            "credential" => changed.credential.issued_at_millis += 1,
            _ => changed.schema = crate::MEMBER_RETURN_SCHEMA.into(),
        }
        assert!(changed.verify(&key.verifying_key()).is_err(), "{field}");
    }
    assert!(presentation.verify(&[9; 32]).is_err());
}
#[test]
fn compact_receipt_checks_exact_local_projection_with_existing_pins_and_limits() {
    let (retained, projection, owner, key) = fixture();
    let mut current = retained.clone();
    current.boot_id = projection.context.boot_id.clone();
    let ack = MemberReturnImageAcknowledgement::new(current, projection.clone());
    let signed =
        attest_member_return_image_acknowledgement(&ack, owner.clone(), [9; 32], |t| key.sign(t))
            .unwrap();
    assert!(signed.receipt_json.len() <= crate::MAX_PICO_ADMISSION_FRAME_BYTES);
    assert_eq!(crate::MAX_ATTESTED_ADMISSION_RECEIPT_BYTES, 16384);
    let (firmware, used): (MemberReturnImageAcknowledgement, usize) =
        serde_json_core::from_slice(signed.receipt_json.as_bytes()).unwrap();
    assert_eq!(used, signed.receipt_json.len());
    firmware.validate_for(&retained, &projection).unwrap();
    signed
        .verify_image_return(&owner, &[9; 32], &retained, &projection)
        .unwrap();
    assert!(signed
        .verify_return(&owner, &[9; 32], &retained, &projection.context)
        .is_err());
    let mut foreign = projection.clone();
    foreign.capability_images.swap(0, 1);
    assert!(signed
        .verify_image_return(&owner, &[9; 32], &retained, &foreign)
        .is_err());
    let mut wrong = owner.clone();
    wrong.boot_id = "boot/stale-owner".into();
    assert!(signed
        .verify_image_return(&wrong, &[9; 32], &retained, &projection)
        .is_err());
    assert!(signed
        .verify_image_return(&owner, &[8; 32], &retained, &projection)
        .is_err());
    let mut altered = signed;
    altered.receipt_json.push(' ');
    assert!(altered
        .verify_image_return(&owner, &[9; 32], &retained, &projection)
        .is_err());
}
#[test]
fn duplicate_excessive_or_invalid_return_basis_cannot_be_attested() {
    let (retained, mut projection, owner, key) = fixture();
    projection.capability_images[1] = projection.capability_images[0];
    assert!(projection.validate().is_err());
    projection.capability_images = (0..17).map(|i| [i; 32]).collect();
    assert!(projection.validate().is_err());
    projection.capability_images.clear();
    let ack = MemberReturnImageAcknowledgement::new(retained, projection);
    // A credential for the old Boot cannot be signed as a current return.
    assert!(
        attest_member_return_image_acknowledgement(&ack, owner, [9; 32], |t| key.sign(t)).is_err()
    );
}
