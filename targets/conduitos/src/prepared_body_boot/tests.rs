use super::*;
use conduit_body::{
    Body, BodyBiographyEvidence, BodyMembership, PREPARED_BIRTH_MEDIA_BYTES,
    PreparedBirthMediaBinding,
};

fn frame(host: &str) -> alloc::vec::Vec<u8> {
    let body = Body::born(
        "source/example".into(),
        "checked/example".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Pine Lantern".into()).unwrap();
    let make = MakeRecord::legacy();
    let media = PreparedBirthMedia::from_evidence(
        &evidence,
        PreparedBirthMediaBinding {
            host_entry: host.into(),
            first_host_entry: "first".into(),
            construction_id: "construction/exact".into(),
            configuration_id: "configuration/exact".into(),
            profile_id: make.profile_id.into(),
            build_id: make.build_id.into(),
            target: make.target.into(),
        },
    )
    .unwrap();
    let mut bytes = alloc::vec![0; PREPARED_BIRTH_MEDIA_BYTES];
    media.encode_into(&mut bytes).unwrap();
    bytes
}

#[test]
fn boot_inspection_requires_valid_frame_exact_image_and_retained_host_role() {
    let bytes = frame("first");
    let make = MakeRecord::legacy();
    let retained = inspect(&bytes, &make).unwrap().unwrap();
    assert_eq!(retained.evidence().friendly_name, "Pine Lantern");
    assert!(retained.evidence().membership.parts.is_empty());
    for wrong in [
        MakeRecord {
            target: "wrong",
            ..make
        },
        MakeRecord {
            profile_id: "wrong",
            ..make
        },
        MakeRecord {
            build_id: "wrong",
            ..make
        },
    ] {
        assert_eq!(
            inspect(&bytes, &wrong),
            Err("prepared-birth-image-binding-mismatch")
        );
    }
    let later = inspect(&frame("later"), &make).unwrap().unwrap();
    assert_eq!(later.role(), conduit_body::PreparedBirthHostRole::LaterHost);
    assert!(later.evidence().membership.parts.is_empty());
    let mut corrupt = bytes;
    corrupt[48] ^= 1;
    assert_eq!(
        inspect(&corrupt, &make),
        Err("prepared-birth-media-invalid")
    );
    assert_eq!(
        inspect(b"CDBIRTH1", &make),
        Err("prepared-birth-media-invalid")
    );
    assert_eq!(inspect(b"CONDUIT_SPORE_MEDIA@1", &make), Ok(None));
}
