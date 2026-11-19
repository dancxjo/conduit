//! Real AdmissionManager witness, without a product durable-return claim.
use super::*;
use crate::{StdHost, StdHostConfig};
use conduit_body::{
    AdmissionManager, AdmissionSigns, BodyId, BodyMembership, HostAdvertisementImageProjection,
    MembershipCredential, SpawnAdmissionProof, SpawnInvitationSecret,
};
use conduit_core::{HostAdvertisement, OfferGeneration, PROTOCOL_VERSION};
fn admitted() -> (
    AdmissionManager,
    BodyMembership,
    HostAdvertisement,
    MembershipCredential,
    [u8; 32],
) {
    let body = serde_json::from_str::<BodyId>(r#""fixture/image-return-body""#).unwrap();
    let mut manager = AdmissionManager::new(body.clone()).unwrap();
    let mut membership = BodyMembership::new(body).unwrap();
    let actual = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/member".into(),
        boot_id: "fixture/member-boot".into(),
        offer_generation: OfferGeneration(3),
        profile: "fixture/member-profile".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
            [9; 32],
            1,
            1000,
        )
        .unwrap();
    let key = invitation.secret.verifying_key();
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
                part_admitted: "fixture/admitted".into(),
                host_attached: "fixture/attached".into(),
                candidate_admitted: "fixture/candidate".into(),
            },
        )
        .unwrap();
    (manager, membership, actual, credential, key)
}
fn identity() -> StartupOwnerIdentity {
    StdHost::new_with_config(StdHostConfig {
        host_id: "fixture/original-owner".into(),
        boot_id: "fixture/owner-boot".into(),
        offer_generation: OfferGeneration(1),
    })
    .prepare_startup_owner_identity()
    .unwrap()
}
fn receipt(
    credential: MembershipCredential,
    actual: HostAdvertisement,
) -> MemberReturnImageAcknowledgement {
    MemberReturnImageAcknowledgement::new(
        credential,
        HostAdvertisementImageProjection {
            context: actual,
            capability_images: vec![[3; 32]],
            authority_grants: vec![],
        },
    )
}
#[test]
fn original_identity_attests_only_typed_current_manager_member_and_retains_owner_pin() {
    let identity = identity();
    let (manager, membership, actual, credential, key) = admitted();
    let witness = manager
        .current_member_continuity(&membership, &credential.part_id, &actual)
        .unwrap();
    assert_eq!(witness.peer().verifying_key, key);
    let receipt = receipt(credential.clone(), actual);
    let signed = identity
        .attest_image_member_return(&receipt, &witness)
        .unwrap();
    assert_eq!(signed.owner, *identity.peer());
    assert_eq!(signed.member_verifying_key, key);
    let verified = signed
        .verify_image_return(identity.peer(), &key, &credential, &receipt.projection)
        .unwrap();
    assert_eq!(verified.credential, credential);
    assert_eq!(verified.projection, receipt.projection);
    let mut wrong_owner = identity.peer().clone();
    wrong_owner.boot_id = "wrong/owner-boot".into();
    assert!(signed
        .verify_image_return(&wrong_owner, &key, &credential, &receipt.projection)
        .is_err());
    assert!(signed
        .verify_image_return(identity.peer(), &[99; 32], &credential, &receipt.projection)
        .is_err());
}
#[test]
fn changed_body_part_host_boot_or_generation_cannot_be_signed_for_a_current_witness() {
    let identity = identity();
    let (manager, membership, actual, credential, _) = admitted();
    let witness = manager
        .current_member_continuity(&membership, &credential.part_id, &actual)
        .unwrap();
    let receipt = receipt(credential, actual);
    for change in 0..6 {
        let mut wrong = receipt.clone();
        match change {
            0 => wrong.credential.body_id = serde_json::from_str(r#""wrong/body""#).unwrap(),
            1 => wrong.credential.part_id = serde_json::from_str(r#""wrong/part""#).unwrap(),
            2 => {
                wrong.credential.host_id = "wrong/member-host".into();
                wrong.projection.context.host_id = wrong.credential.host_id.clone();
            }
            3 => {
                wrong.credential.boot_id = "wrong/member-boot".into();
                wrong.projection.context.boot_id = wrong.credential.boot_id.clone();
            }
            4 => wrong.projection.context.offer_generation = OfferGeneration(4),
            _ => wrong.projection.context.protocol_version = 0,
        }
        assert!(
            identity
                .attest_image_member_return(&wrong, &witness)
                .is_err(),
            "change {change}"
        );
    }
    // A public advertisement cannot manufacture the manager witness.
    let absent = AdmissionManager::new(membership.body_id.clone()).unwrap();
    assert!(absent
        .current_member_continuity(
            &membership,
            &receipt.credential.part_id,
            &receipt.projection.context
        )
        .is_err());
}
