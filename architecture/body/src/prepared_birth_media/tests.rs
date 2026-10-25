use super::*;
use crate::{Body, BodyMembership};
fn evidence() -> BodyBiographyEvidence {
    let body = Body::born(
        "source/creche".into(),
        "checked/creche".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    BodyBiographyEvidence::born(body, membership, "Pine Lantern".into()).unwrap()
}
fn binding(host: &str) -> PreparedBirthMediaBinding {
    PreparedBirthMediaBinding {
        host_entry: host.into(),
        first_host_entry: "desk".into(),
        construction_id: "construction/exact".into(),
        configuration_id: "configuration/exact".into(),
        profile_id: "profile/exact".into(),
        build_id: "build/exact".into(),
        target: "target/exact".into(),
    }
}
fn frame(media: &PreparedBirthMedia) -> Vec<u8> {
    let mut bytes = vec![0xaa; PREPARED_BIRTH_MEDIA_BYTES];
    assert_eq!(media.encode_into(&mut bytes).unwrap(), bytes.len());
    bytes
}
#[test]
fn two_host_frames_retain_one_exact_birth_and_name_with_distinct_roles() {
    let birth = evidence();
    let first = PreparedBirthMedia::from_evidence(&birth, binding("desk")).unwrap();
    let later = PreparedBirthMedia::from_evidence(&birth, binding("satellite")).unwrap();
    for (media, role) in [
        (first, PreparedBirthHostRole::FirstHost),
        (later, PreparedBirthHostRole::LaterHost),
    ] {
        let restored = PreparedBirthMedia::decode(&frame(&media)).unwrap();
        assert_eq!(restored.evidence(), &birth);
        assert_eq!(restored.evidence().friendly_name, "Pine Lantern");
        assert_eq!(restored.evidence().body_id, birth.body_id);
        assert_eq!(restored.role(), role);
        assert!(restored.evidence().membership.parts.is_empty());
        assert_eq!(restored.binding(), media.binding());
    }
}
#[test]
fn corrupt_truncated_oversized_and_noncanonical_frames_are_refused() {
    let valid = frame(&PreparedBirthMedia::from_evidence(&evidence(), binding("desk")).unwrap());
    let mut corrupt = valid.clone();
    corrupt[48] ^= 1;
    assert_eq!(
        PreparedBirthMedia::decode(&corrupt),
        Err(PreparedBirthMediaError::InvalidDigest)
    );
    assert!(PreparedBirthMedia::decode(&valid[..valid.len() - 1]).is_err());
    let mut oversized = valid.clone();
    oversized.push(0);
    assert!(PreparedBirthMedia::decode(&oversized).is_err());
    let mut tail = valid.clone();
    tail[PREPARED_BIRTH_MEDIA_BYTES - 1] = 1;
    assert_eq!(
        PreparedBirthMedia::decode(&tail),
        Err(PreparedBirthMediaError::NonCanonical)
    );
    let mut reserved = valid.clone();
    reserved[10] = 1;
    assert!(PreparedBirthMedia::decode(&reserved).is_err());
    let mut version = valid;
    version[8] = 2;
    assert_eq!(
        PreparedBirthMedia::decode(&version),
        Err(PreparedBirthMediaError::UnsupportedVersion)
    );
}
#[test]
fn lifecycle_history_and_nonempty_membership_cannot_be_prepared_birth() {
    let mut member = evidence();
    let mut membership = member.membership.clone();
    let part = crate::PartId::bind(&member.body_id, "host/member", 1).unwrap();
    let change = membership
        .admit(
            &member.body_id,
            membership.revision,
            part,
            crate::MembershipProofId::bind("proof/member").unwrap(),
            "sign/member".into(),
        )
        .unwrap();
    member
        .append_membership_events(membership, &[(change, 2)])
        .unwrap();
    member.validate().unwrap();
    assert!(!member.membership.parts.is_empty());
    assert_eq!(
        PreparedBirthMedia::from_evidence(&member, binding("desk")),
        Err(PreparedBirthMediaError::InvalidBirth)
    );
    let pristine = evidence();
    let (awake, _) = pristine.body.wake(2, "sign/woke".into()).unwrap();
    let mut woke = pristine.clone();
    woke.body = awake;
    assert_eq!(
        PreparedBirthMedia::from_evidence(&woke, binding("desk")),
        Err(PreparedBirthMediaError::InvalidBirth)
    );
    let mut history = pristine;
    history.body.sign_ids.push("sign/extra".into());
    assert_eq!(
        PreparedBirthMedia::from_evidence(&history, binding("desk")),
        Err(PreparedBirthMediaError::InvalidBirth)
    );
}
#[test]
fn binding_bounds_and_small_output_are_refused() {
    let mut bad = binding("desk");
    bad.target = "x".repeat(MAX_PREPARED_BIRTH_BINDING_BYTES + 1);
    assert_eq!(
        PreparedBirthMedia::from_evidence(&evidence(), bad),
        Err(PreparedBirthMediaError::InvalidBinding)
    );
    let media = PreparedBirthMedia::from_evidence(&evidence(), binding("desk")).unwrap();
    assert_eq!(
        media.encode_into(&mut [0; 128]),
        Err(PreparedBirthMediaError::CapacityExceeded)
    );
}

#[test]
fn overlong_postcard_encoding_is_not_a_second_canonical_frame() {
    let mut bytes =
        frame(&PreparedBirthMedia::from_evidence(&evidence(), binding("desk")).unwrap());
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    // The first payload field is the short retained Body ID. Re-encode its
    // length using an unnecessary extra varint byte, then reseal the checksum.
    assert!(bytes[48] < 128);
    bytes.copy_within(49..48 + length, 50);
    bytes[48] |= 128;
    bytes[49] = 0;
    bytes[12..16].copy_from_slice(&((length + 1) as u32).to_le_bytes());
    let digest = Sha256::digest(&bytes[48..49 + length]);
    bytes[16..48].copy_from_slice(&digest);
    assert!(PreparedBirthMedia::decode(&bytes).is_err());
}
