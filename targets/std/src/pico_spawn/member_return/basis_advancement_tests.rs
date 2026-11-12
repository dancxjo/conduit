//! CPU pinned-owner receipt validation; no device or recovery authority claim.
use super::*;
use conduit_body::{attest_member_return_image_acknowledgement, HostAdvertisementImageProjection};
fn projection(ad: HostAdvertisement) -> HostAdvertisementImageProjection {
    HostAdvertisementImageProjection {
        context: ad,
        authority_grants: vec![],
        capability_images: vec![[3; 32], [4; 32]],
    }
}
fn receipt(
    basis: &PicoMemberReturnBasis,
    projection: &HostAdvertisementImageProjection,
) -> SignedPortableAdmissionReceipt {
    let mut current = basis.credential.clone();
    current.credential_id = id("credential/current");
    current.boot_id = projection.context.boot_id.clone();
    current.issued_at_millis += 1;
    let ack = MemberReturnImageAcknowledgement::new(current, projection.clone());
    let owner_key = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    attest_member_return_image_acknowledgement(
        &ack,
        basis.owner.clone(),
        basis.member_verifying_key,
        |t| owner_key.sign(t),
    )
    .unwrap()
}
#[test]
fn old_basis_remains_strict_until_exact_verified_receipt_advances_only_credential() {
    let (basis, ad, key) = fixture();
    let actual = projection(ad);
    let signed = receipt(&basis, &actual);
    let acknowledged: MemberReturnImageAcknowledgement =
        serde_json::from_str(&signed.receipt_json).unwrap();
    let claim = MemberReturnPresentation {
        kind: "Advertise".into(),
        schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
        credential: acknowledged.credential.clone(),
        projection: actual.clone(),
        signature: key
            .sign(
                &conduit_body::member_return_image_transcript(&acknowledged.credential, &actual)
                    .unwrap(),
            )
            .to_vec(),
    };
    assert!(basis.check_presentation(&claim).is_err());
    let advanced = basis
        .advance_from_verified_receipt(&signed, &actual)
        .unwrap();
    assert!(
        basis.check_presentation(&claim).is_err(),
        "original basis is unchanged"
    );
    advanced.check_presentation(&claim).unwrap();
    assert_eq!(advanced.credential, acknowledged.credential);
    assert_eq!(advanced.expected_profile, basis.expected_profile);
    assert_eq!(advanced.owner, basis.owner);
    assert_eq!(advanced.member_verifying_key, basis.member_verifying_key);
    assert_eq!(
        advanced
            .advance_from_verified_receipt(&signed, &actual)
            .unwrap()
            .credential,
        advanced.credential,
        "same verified receipt is idempotent"
    );
}
#[test]
fn wrong_owner_member_key_relationship_profile_and_full_projection_refuse() {
    let (basis, ad, _) = fixture();
    let actual = projection(ad);
    let signed = receipt(&basis, &actual);
    for case in 0..6 {
        let mut changed = basis.clone();
        match case {
            0 => changed.owner.boot_id = "boot/foreign-owner".into(),
            1 => {
                changed.owner.verifying_key = SpawnInvitationSecret::from_csprng_bytes([8; 32])
                    .unwrap()
                    .verifying_key()
            }
            2 => {
                changed.member_verifying_key = SpawnInvitationSecret::from_csprng_bytes([6; 32])
                    .unwrap()
                    .verifying_key()
            }
            3 => changed.credential.body_id = id("body/foreign"),
            4 => changed.credential.part_id = id("part/foreign"),
            _ => changed.credential.host_id = "host/foreign".into(),
        }
        assert!(changed
            .advance_from_verified_receipt(&signed, &actual)
            .is_err());
    }
    for case in 0..6 {
        let mut changed = actual.clone();
        match case {
            0 => changed.context.profile = "profile/foreign".into(),
            1 => changed.context.host_id = "host/foreign".into(),
            2 => changed.context.boot_id = "boot/foreign".into(),
            3 => changed.context.offer_generation.0 += 1,
            4 => changed.capability_images[0][0] ^= 1,
            _ => changed.capability_images.swap(0, 1),
        }
        assert!(basis
            .advance_from_verified_receipt(&signed, &changed)
            .is_err());
    }
    let mut changed = actual.clone();
    changed.context.profile = "profile/foreign".into();
    let wrong_profile = receipt(&basis, &changed);
    assert!(
        basis
            .advance_from_verified_receipt(&wrong_profile, &changed)
            .is_err(),
        "even genuine owner signature cannot replace independently pinned profile"
    );
    // Re-sign a structurally valid foreign relationship with the RIGHT owner;
    // equality checks must still reject it, independently of signature checks.
    for case in 0..3 {
        let mut foreign = basis.clone();
        let mut context = actual.clone();
        match case {
            0 => foreign.credential.body_id = id("body/foreign"),
            1 => foreign.credential.part_id = id("part/foreign"),
            _ => {
                foreign.credential.host_id = "host/foreign".into();
                context.context.host_id = "host/foreign".into();
            }
        }
        let wrong = receipt(&foreign, &context);
        assert!(basis
            .advance_from_verified_receipt(&wrong, &context)
            .is_err());
    }
}
