use super::*;
use crate::SpawnInvitationSecret;
use crate::MAX_LIFECYCLE_ID_BYTES;
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
        authority_grants: vec![],
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
    projection.capability_images = (0..=MAX_MEMBER_RETURN_IMAGES)
        .map(|i| [i as u8; 32])
        .collect();
    assert!(projection.validate().is_err());
    projection.capability_images.clear();
    let ack = MemberReturnImageAcknowledgement::new(retained, projection);
    // A credential for the old Boot cannot be signed as a current return.
    assert!(
        attest_member_return_image_acknowledgement(&ack, owner, [9; 32], |t| key.sign(t)).is_err()
    );
}

fn grant(projection: &HostAdvertisementImageProjection) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: "grant/independent".into(),
        contract_id: "authority/i2c".into(),
        host_call_contract_id: "call/i2c".into(),
        subject_kind: "kind/i2c".into(),
        host_id: projection.context.host_id.clone(),
        boot_id: projection.context.boot_id.clone(),
        capability_id: "capability/i2c".into(),
    }
}

#[test]
fn attributed_original_grants_are_signed_and_acknowledged_exactly() {
    let (retained, mut projection, _, key) = fixture();
    assert!(projection.authority_grants.is_empty());
    projection.validate().unwrap(); // No inferred authority, despite sixteen images.
    let empty = member_return_image_transcript(&retained, &projection).unwrap();
    projection.authority_grants.push(grant(&projection));
    let transcript = member_return_image_transcript(&retained, &projection).unwrap();
    assert_ne!(empty, transcript);
    let presentation = MemberReturnImagePresentation {
        kind: "Advertise".into(),
        schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
        credential: retained.clone(),
        projection: projection.clone(),
        signature: key.sign(&transcript).into(),
    };
    presentation.verify(&key.verifying_key()).unwrap();
    for change in 0..3 {
        let mut changed = presentation.clone();
        match change {
            0 => changed.projection.authority_grants.clear(),
            1 => changed.projection.authority_grants[0].grant_id = "grant/substitute".into(),
            _ => changed.projection.authority_grants[0].subject_kind = "kind/substitute".into(),
        }
        assert!(changed.verify(&key.verifying_key()).is_err());
    }
    let mut current = retained.clone();
    current.boot_id = projection.context.boot_id.clone();
    let ack = MemberReturnImageAcknowledgement::new(current, projection.clone());
    ack.validate_for(&retained, &projection).unwrap();
    projection.authority_grants.clear();
    assert!(ack.validate_for(&retained, &projection).is_err());
}

#[test]
fn grant_scope_duplicates_and_finite_identifier_limits_refuse() {
    let (_, projection, _, _) = fixture();
    let valid = grant(&projection);
    for change in 0..7 {
        let mut changed = projection.clone();
        let mut grant = valid.clone();
        match change {
            0 => grant.host_id = "host/other".into(),
            1 => grant.boot_id = "boot/other".into(),
            2 => grant.grant_id = "".into(),
            3 => grant.subject_kind = "x".repeat(MAX_LIFECYCLE_ID_BYTES + 1).into(),
            4 => changed.authority_grants = vec![valid.clone(); MAX_MEMBER_RETURN_IMAGE_GRANTS],
            5 => changed.authority_grants.push(valid.clone()),
            _ => {
                changed.authority_grants.push(valid.clone());
                grant.grant_id = "grant/other-id-same-scope".into();
            }
        }
        changed.authority_grants.push(grant);
        assert!(changed.validate().is_err(), "mutation {change}");
    }
    let mut excessive = projection;
    for i in 0..5 {
        let mut grant = valid.clone();
        grant.grant_id = alloc::format!("{i}{}", "g".repeat(255)).into();
        grant.contract_id = "c".repeat(256).into();
        grant.host_call_contract_id = "h".repeat(256).into();
        grant.subject_kind = "s".repeat(256).into();
        grant.capability_id = alloc::format!("{i}{}", "p".repeat(255)).into();
        excessive.authority_grants.push(grant);
    }
    assert!(excessive.validate().is_err());
}

#[test]
fn caller_funded_transcript_preserves_existing_signature_bytes() {
    let (mut retained, mut projection, _, _) = fixture();
    for text in [
        "plain",
        "quote\"slash\\newline\n",
        "unicode/温度",
        "control\u{0001}",
    ] {
        retained.credential_id =
            serde_json::from_value(serde_json::Value::String(text.into())).unwrap();
        projection.context.profile = text.into();
        let mut scratch = [0; 8192];
        assert_eq!(
            member_return_image_transcript(&retained, &projection).unwrap(),
            member_return_image_transcript_into(&retained, projection.borrowed(), &mut scratch)
                .unwrap()
        );
    }
    assert!(
        member_return_image_transcript_into(&retained, projection.borrowed(), &mut [0; 8]).is_err()
    );
}

#[test]
fn incremental_acknowledgement_preserves_semantic_projection_checks() {
    use serde::de::DeserializeSeed;
    let (retained, projection, _, _) = fixture();
    let mut current = retained.clone();
    current.boot_id = projection.context.boot_id.clone();
    let ack = MemberReturnImageAcknowledgement::new(current, projection.clone());
    let original = serde_json::to_value(&ack).unwrap();
    let check = |value: &serde_json::Value| {
        let bytes = serde_json::to_vec(value).unwrap();
        let mut decoder = serde_json::Deserializer::from_slice(&bytes);
        let result = MemberReturnImageAcknowledgementSeed {
            retained: &retained,
            actual: projection.borrowed(),
        }
        .deserialize(&mut decoder);
        let owned = serde_json::from_slice::<MemberReturnImageAcknowledgement>(&bytes)
            .ok()
            .filter(|ack| ack.validate_for(&retained, &projection).is_ok());
        let mut core_decoder = serde_json_core::de::Deserializer::new(&bytes, None);
        let core_result = MemberReturnImageAcknowledgementSeed {
            retained: &retained,
            actual: projection.borrowed(),
        }
        .deserialize(&mut core_decoder);
        assert_eq!(core_result.is_ok(), owned.is_some());
        if core_result.is_ok() {
            assert_eq!(core_decoder.end().unwrap(), bytes.len());
        }
        assert_eq!(result.is_ok(), owned.is_some());
        if let Ok(credential) = result {
            assert_eq!(credential, owned.unwrap().credential);
        }
    };
    // serde_json::Value emits sorted object keys rather than struct declaration
    // order. Omitted optional Host fields retain their ordinary defaults.
    check(&original);
    let mut omitted = original.clone();
    omitted["projection"]["context"]
        .as_object_mut()
        .unwrap()
        .remove("declared_name");
    check(&omitted);
    for field in ["kind", "schema", "credential", "context", "images", "extra"] {
        let mut changed = original.clone();
        match field {
            "kind" => changed["kind"] = "Advertise".into(),
            "schema" => changed["schema"] = "other".into(),
            "credential" => changed["credential"]["boot_id"] = "boot/other".into(),
            "context" => changed["projection"]["context"]["boot_id"] = "boot/other".into(),
            "images" => changed["projection"]["capability_images"]
                .as_array_mut()
                .unwrap()
                .reverse(),
            _ => changed["projection"]["unexpected"] = true.into(),
        }
        check(&changed);
    }
}

#[test]
fn referenced_original_grants_preserve_transcript_and_acknowledgement_laws() {
    use serde::de::DeserializeSeed;
    let (retained, mut projection, _, _) = fixture();
    projection.authority_grants.push(grant(&projection));
    let references: Vec<_> = projection.authority_grants.iter().collect();
    let borrowed = BorrowedHostAdvertisementImageProjection {
        context: &projection.context,
        capability_images: &projection.capability_images,
        authority_grants: &references,
    };
    let mut scratch = [0; 8192];
    assert_eq!(
        member_return_image_transcript(&retained, &projection).unwrap(),
        member_return_image_transcript_into(&retained, borrowed, &mut scratch).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&projection).unwrap(),
        serde_json::to_vec(&borrowed).unwrap()
    );
    let mut current = retained.clone();
    current.boot_id = projection.context.boot_id.clone();
    let ack = MemberReturnImageAcknowledgement::new(current.clone(), projection.clone());
    let original = serde_json::to_value(&ack).unwrap();
    for altered in [false, true] {
        let mut value = original.clone();
        if altered {
            value["projection"]["authority_grants"][0]["grant_id"] = "grant/substitute".into();
        }
        let bytes = serde_json::to_vec(&value).unwrap();
        let mut decoder = serde_json_core::de::Deserializer::new(&bytes, None);
        let result = MemberReturnImageAcknowledgementSeed {
            retained: &retained,
            actual: borrowed,
        }
        .deserialize(&mut decoder);
        assert_eq!(result.is_ok(), !altered);
        if let Ok(credential) = result {
            assert_eq!(credential, current);
            assert_eq!(decoder.end().unwrap(), bytes.len());
        }
    }
    let duplicate = [
        &projection.authority_grants[0],
        &projection.authority_grants[0],
    ];
    assert!(BorrowedHostAdvertisementImageProjection {
        authority_grants: &duplicate,
        ..borrowed
    }
    .validate()
    .is_err());
}
