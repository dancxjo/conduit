use super::*;
use crate::{
    AdmissionSigns, HostAdvertisementImageProjection, SpawnAdmissionProof, SpawnInvitationSecret,
    MEMBER_RETURN_IMAGES_SCHEMA,
};
use alloc::vec;
use conduit_core::{HostAdvertisement, OfferGeneration, PROTOCOL_VERSION};
use ed25519_dalek::{Signer, SigningKey};

fn fixture() -> (
    AdmissionManager,
    BodyMembership,
    MemberReturnImagePresentation,
) {
    let body = crate::BodyId::bound("fixture/body".into());
    let mut manager = AdmissionManager::new(body.clone()).unwrap();
    let mut membership = BodyMembership::new(body).unwrap();
    let actual = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/old-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/profile".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([1; 32]).unwrap(),
            [8; 32],
            1,
            1000,
        )
        .unwrap();
    let proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &actual.host_id,
            &actual.boot_id,
            actual.offer_generation,
        )),
    };
    let credential = manager
        .complete_spawn(
            &mut membership,
            &actual,
            &proof,
            2,
            AdmissionSigns {
                part_admitted: "fixture/part-sign".into(),
                host_attached: "fixture/host-sign".into(),
                candidate_admitted: "fixture/candidate-sign".into(),
            },
        )
        .unwrap();
    let mut context = actual;
    context.boot_id = "fixture/new-boot".into();
    let mut presentation = MemberReturnImagePresentation {
        signature: vec![],
        kind: "Advertise".into(),
        schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
        credential,
        projection: HostAdvertisementImageProjection {
            context,
            capability_images: vec![[3; 32]],
            authority_grants: vec![],
        },
    };
    sign(&mut presentation, 1);
    (manager, membership, presentation)
}
fn sign(p: &mut MemberReturnImagePresentation, seed: u8) {
    p.signature = SigningKey::from_bytes(&[seed; 32])
        .sign(&crate::member_return_image_transcript(&p.credential, &p.projection).unwrap())
        .to_bytes()
        .to_vec();
}

#[test]
fn original_key_attributes_prior_credential_without_admitting_new_boot() {
    let (manager, membership, presentation) = fixture();
    let before = manager.clone();
    assert_eq!(
        manager.verify_member_return_image_presentation(&membership, &presentation),
        Ok(())
    );
    assert_eq!(manager, before);
    assert_ne!(
        membership.parts[0].current.as_ref().unwrap().boot_id,
        presentation.projection.context.boot_id
    );
    let mut stale = presentation.clone();
    stale.credential.credential_id =
        crate::MembershipCredentialId::bound("fixture/older-credential".into());
    sign(&mut stale, 1);
    assert_eq!(
        manager.verify_member_return_image_presentation(&membership, &stale),
        Ok(())
    );
    assert_ne!(
        manager.receipts.last().unwrap().credential,
        stale.credential
    );
}

#[test]
fn wrong_signer_body_part_host_or_shape_cannot_attribute_images() {
    for change in 0..6 {
        let (manager, membership, mut p) = fixture();
        match change {
            0 => {}
            1 => p.credential.body_id = crate::BodyId::bound("fixture/wrong-body".into()),
            2 => p.credential.part_id = crate::PartId::bound("fixture/wrong-part".into()),
            3 => {
                p.credential.host_id = "fixture/wrong-host".into();
                p.projection.context.host_id = p.credential.host_id.clone();
            }
            4 => p.projection.context.host_id = "fixture/wrong-host".into(),
            _ => p.schema = "fixture/wrong-schema".into(),
        }
        sign(&mut p, if change == 0 { 2 } else { 1 });
        assert!(manager
            .verify_member_return_image_presentation(&membership, &p)
            .is_err());
    }
}

#[test]
fn absent_nonadmitted_duplicate_parts_or_keys_refuse() {
    for change in 0..6 {
        let (mut manager, mut membership, p) = fixture();
        match change {
            0 => membership.parts.clear(),
            1 => membership.parts[0].state = MembershipState::Revoked,
            2 => membership.parts.push(membership.parts[0].clone()),
            3 => manager
                .continuity_keys
                .push(manager.continuity_keys[0].clone()),
            4 => manager.continuity_keys.clear(),
            _ => membership.body_id = crate::BodyId::bound("fixture/wrong-body".into()),
        }
        assert!(manager
            .verify_member_return_image_presentation(&membership, &p)
            .is_err());
    }
}
