//! Owned IPC/decoder input becomes the same retained pending allocation.
use conduit_body::{
    PortableSpawnAdmissionRequest, SpawnInvitationSecret, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_std_host::{StdHost, StdHostConfig};

#[test]
fn owned_return_moves_offer_backing_and_replay_keeps_original_pending_owner() {
    let (mut owner, root) = crate::durable_host::owner::member_return_owner_fixture();
    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let guest = StdHost::new_with_config(StdHostConfig {
        host_id: "host/owned-return".into(),
        boot_id: "boot/owned-return/1".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let mut offer = guest.advertisement().clone();
    let key = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: offer.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &offer.host_id,
                &offer.boot_id,
                offer.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let prior = owner
        .admit_invited(&root, request, "host/owned-return")
        .unwrap()
        .credential;
    owner.member_leave(&root, &prior).unwrap();
    offer.boot_id = "boot/owned-return/2".into();
    offer.offer_generation.0 += 1;
    // Spare backing is observable ownership, not an additional resource grant.
    offer.resources.reserve_exact(16);
    let resources = offer.resources.as_ptr();
    let profile = offer.profile.0.as_ptr();
    let replay = offer.clone();
    let challenge = owner.begin_member_return_owned(&prior, offer).unwrap();
    let pending = owner.pending_member_return.as_ref().unwrap();
    assert_eq!(pending.advertisement.resources.as_ptr(), resources);
    assert_eq!(pending.advertisement.profile.0.as_ptr(), profile);
    assert_eq!(
        owner
            .begin_member_return_owned(&prior, replay.clone())
            .unwrap(),
        challenge
    );
    let pending = owner.pending_member_return.as_ref().unwrap();
    assert_eq!(pending.advertisement.resources.as_ptr(), resources);
    assert_eq!(pending.advertisement.profile.0.as_ptr(), profile);
    let mut conflict = replay;
    conflict.boot_id = "boot/conflicting-return".into();
    assert!(owner.begin_member_return_owned(&prior, conflict).is_err());
    assert_eq!(
        owner
            .pending_member_return
            .as_ref()
            .unwrap()
            .advertisement
            .resources
            .as_ptr(),
        resources
    );
    std::fs::remove_dir_all(root).unwrap();
}
