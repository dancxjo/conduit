//! Actual owner state ordering, not live deployment eligibility or MCU proof.
use super::*;
use conduit_body::{
    member_return_image_transcript, PortableSpawnAdmissionRequest, SpawnInvitationSecret,
    MEMBER_RETURN_IMAGES_SCHEMA, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{HostAdvertisement, OfferGeneration, PROTOCOL_VERSION};
use std::path::PathBuf;
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::Fixture;
#[test]
fn exact_image_return_persists_core_before_retaining_and_replaying_the_same_signed_receipt() {
    let mut f = Fixture::new("image-return-good");
    let challenge = f.begin().unwrap();
    assert_eq!(f.begin().unwrap(), challenge);
    let before = f.owner.session.evidence().clone();
    let signed = f.complete(&challenge).unwrap();
    let committed = f.owner.session.evidence().clone();
    assert_ne!(committed, before);
    assert_eq!(state::load(&f.root).unwrap().unwrap(), committed);
    let ack = signed
        .verify_image_return(
            &f.owner.member_carrier_peer().unwrap(),
            &f.key.verifying_key(),
            &f.prior,
            &f.projection,
        )
        .unwrap();
    assert_eq!(ack.credential.part_id, f.prior.part_id);
    assert_eq!(ack.credential.boot_id, f.projection.context.boot_id);
    let encoded = serde_json::to_vec(&signed).unwrap();
    assert_eq!(
        crate::durable_host::bounded_read(
            &receipt_path(&f.root, &encoded).unwrap(),
            MAX_STORED_RECEIPT_BYTES
        )
        .unwrap(),
        encoded
    );
    let replay = f.complete(&challenge).unwrap();
    assert_eq!(serde_json::to_vec(&replay).unwrap(), encoded);
    assert_eq!(f.owner.session.evidence(), &committed);
    assert_eq!(
        std::fs::read_dir(f.root.join("body/member-return-receipts"))
            .unwrap()
            .count(),
        1
    );
}
#[test]
fn core_persistence_failure_produces_no_attested_receipt_or_membership_change() {
    let mut f = Fixture::new("image-return-core-fault");
    let challenge = f.begin().unwrap();
    let before = f.owner.session.evidence().clone();
    let manager = f.owner.admissions.clone();
    let blocked = f.root.join("blocked-root");
    std::fs::write(&blocked, b"occupied").unwrap();
    let proof = f.proof(&challenge);
    assert!(f
        .owner
        .fixture_complete_image_member_return(&blocked, &proof)
        .is_err());
    assert_eq!(f.owner.session.evidence(), &before);
    assert_eq!(f.owner.admissions, manager);
    assert!(!f.root.join("body/member-return-receipts").exists());
    f.complete(&challenge).unwrap();
}
#[test]
fn post_commit_receipt_failure_is_explicit_unknown_delivery_and_exact_retry_recovers() {
    let mut f = Fixture::new("image-return-receipt-fault");
    let challenge = f.begin().unwrap();
    let before = f.owner.session.evidence().clone();
    // Exercise the real boundary after Core durability, before receipt storage.
    let proof = f.proof(&challenge);
    f.owner.complete_member_return(&f.root, &proof).unwrap();
    let committed = f.owner.session.evidence().clone();
    assert_ne!(committed, before);
    let blocked = f.root.join("body/member-return-receipts");
    std::fs::write(&blocked, b"occupied").unwrap();
    let error = f.complete(&challenge).err().unwrap();
    assert!(error.contains("member returned; signed receipt retention failed"));
    assert_eq!(state::load(&f.root).unwrap().unwrap(), committed);
    std::fs::remove_file(blocked).unwrap();
    let signed = f.complete(&challenge).unwrap();
    assert_eq!(f.owner.session.evidence(), &committed);
    signed
        .verify_image_return(
            &f.owner.member_carrier_peer().unwrap(),
            &f.key.verifying_key(),
            &f.prior,
            &f.projection,
        )
        .unwrap();
}
#[test]
fn same_decoded_offers_cannot_replace_cached_signed_images_or_grant_order() {
    let mut f = Fixture::new("image-return-exact-pending");
    let challenge = f.begin().unwrap();
    let original = f
        .owner
        .pending_member_return
        .as_ref()
        .unwrap()
        .image_projection
        .clone();
    let mut images = f.images.clone();
    images[0][9] = 0x9c; // Different valid zlib header, same stored data.
    let mut projection = f.projection.clone();
    projection.capability_images[0] = Sha256::digest(&images[0]).into();
    let changed = f.signed(projection);
    assert!(f
        .owner
        .fixture_begin_image_member_return(changed, &images)
        .is_err());
    let mut projection = f.projection.clone();
    projection.authority_grants = f.grants();
    let changed = f.signed(projection);
    assert!(f
        .owner
        .fixture_begin_image_member_return(changed, &f.images)
        .is_err());
    assert!(
        f.owner
            .pending_member_return
            .as_ref()
            .unwrap()
            .image_projection
            == original
    );
    assert_eq!(
        f.owner.pending_member_return.as_ref().unwrap().challenge,
        challenge
    );
    f.complete(&challenge).unwrap();
    let mut projection = f.projection.clone();
    projection.authority_grants = f.grants();
    let changed = f.signed(projection);
    assert!(f
        .owner
        .fixture_begin_image_member_return(changed, &f.images)
        .is_err());
}
#[test]
fn an_already_completed_ordinary_return_cannot_be_retrofitted_as_image_return() {
    let mut f = Fixture::new("image-return-no-retrofit");
    let maximum = preflight(&f.projection, &f.images).unwrap();
    let actual = resolve(&f.projection, &f.images, maximum).unwrap();
    let challenge = f.owner.begin_member_return(&f.prior, &actual).unwrap();
    let proof = f.proof(&challenge);
    f.owner.complete_member_return(&f.root, &proof).unwrap();
    let before = f.owner.session.evidence().clone();
    assert!(f.begin().is_err());
    assert!(f.complete(&challenge).is_err());
    assert!(f
        .owner
        .pending_member_return
        .as_ref()
        .unwrap()
        .image_projection
        .is_none());
    assert_eq!(f.owner.session.evidence(), &before);
}
#[test]
fn signatures_and_all_image_extents_refuse_before_decode_or_partial_pending_admission() {
    let mut f = Fixture::new("image-return-bounds");
    let mut bad = f.presentation();
    bad.signature[0] ^= 1;
    let images = vec![vec![0]]; // Even malformed image data is not decoded before signature verification.
    let error = f
        .owner
        .fixture_begin_image_member_return(bad, &images)
        .err()
        .unwrap();
    assert!(error.starts_with("member image presentation:"));
    assert!(f.owner.pending_member_return.is_none());
    for expanded in [MAX_EXPANDED_OFFER_BYTES + 1, u32::MAX as usize] {
        let mut images = f.images.clone();
        images[0][4..8].copy_from_slice(&(expanded as u32).to_le_bytes());
        let mut p = f.projection.clone();
        p.capability_images[0] = Sha256::digest(&images[0]).into();
        let changed = f.signed(p);
        assert!(f
            .owner
            .fixture_begin_image_member_return(changed, &images)
            .is_err());
        assert!(f.owner.pending_member_return.is_none());
    }
    let huge = vec![vec![1; MAX_IMAGE_BYTES + 1]];
    let mut p = f.projection.clone();
    p.capability_images[0] = Sha256::digest(&huge[0]).into();
    let changed = f.signed(p);
    assert!(f
        .owner
        .fixture_begin_image_member_return(changed, &huge)
        .is_err());
    assert!(f.owner.pending_member_return.is_none());
    // Three individually bounded expanded claims must also respect the
    // aggregate ceiling before any decoder allocation.
    let images: Vec<Vec<u8>> = (0..3)
        .map(|index| {
            let mut image = f.images[0].clone();
            image[4..8].copy_from_slice(&(200_000_u32 + index).to_le_bytes());
            image
        })
        .collect();
    let mut p = f.projection.clone();
    p.capability_images = images
        .iter()
        .map(|image| Sha256::digest(image).into())
        .collect();
    let changed = f.signed(p);
    assert!(f
        .owner
        .fixture_begin_image_member_return(changed, &images)
        .is_err());
    assert!(f.owner.pending_member_return.is_none());
    f.owner.member_carrier_identity = None;
    assert!(f.begin().is_err());
    assert!(f.owner.pending_member_return.is_none());
}

#[test]
fn receipt_retry_refuses_full_offer_drift_with_unchanged_member_incarnation() {
    let mut f = Fixture::new("image-return-offer-drift");
    let challenge = f.begin().unwrap();
    f.complete(&challenge).unwrap();
    let committed = f.owner.session.evidence().clone();
    f.owner
        .pending_member_return
        .as_mut()
        .unwrap()
        .advertisement
        .capabilities[0]
        .implementation
        .artifact_id = "fixture/changed-artifact".into();
    let error = f.complete(&challenge).err().unwrap();
    assert!(error.contains("full offer differs"));
    assert_eq!(f.owner.session.evidence(), &committed);
}

#[test]
fn changed_retained_grants_refuse_before_core_completion() {
    let mut f = Fixture::new("image-return-grant-drift");
    let challenge = f.begin().unwrap();
    let before = f.owner.session.evidence().clone();
    f.owner
        .pending_member_return
        .as_mut()
        .unwrap()
        .image_projection
        .as_mut()
        .unwrap()
        .projection
        .authority_grants
        .reverse();
    let error = f.complete(&challenge).err().unwrap();
    assert!(error.contains("projection/grants differ from retained signed evidence"));
    assert_eq!(f.owner.session.evidence(), &before);
    assert!(!f.root.join("body/member-return-receipts").exists());
}

#[test]
fn oversized_signed_projection_refuses_before_pending_or_core_mutation() {
    let mut f = Fixture::new("image-return-oversized-receipt");
    let mut projection = f.projection.clone();
    projection
        .context
        .resources
        .push(conduit_core::resource_offer(
            &format!("fixture/{}", "x".repeat(17 * 1024)),
            "fixture/memory",
            4096,
        ));
    projection.validate().unwrap();
    let presentation = f.signed(projection);
    presentation.verify(&f.key.verifying_key()).unwrap();
    assert!(serde_json::to_vec(&presentation.projection).unwrap().len() > 16 * 1024);
    let before = f.owner.session.evidence().clone();
    let manager = f.owner.admissions.clone();
    let error = f
        .owner
        .fixture_begin_image_member_return(presentation, &f.images)
        .err()
        .unwrap();
    assert!(error.contains("receipt carriage bound"));
    assert!(f.owner.pending_member_return.is_none());
    assert_eq!(f.owner.session.evidence(), &before);
    assert_eq!(f.owner.admissions, manager);
}

#[test]
fn full_receipt_store_refuses_before_committing_a_new_return() {
    let mut f = Fixture::new("image-return-receipt-capacity");
    let challenge = f.begin().unwrap();
    let directory = f.root.join("body/member-return-receipts");
    std::fs::create_dir_all(&directory).unwrap();
    for index in 0..MAX_RECEIPTS {
        std::fs::write(directory.join(format!("{index}.json")), b"fixture").unwrap();
    }
    let before = f.owner.session.evidence().clone();
    assert!(f
        .complete(&challenge)
        .err()
        .unwrap()
        .contains("retention capacity exhausted"));
    assert_eq!(f.owner.session.evidence(), &before);
    assert!(f
        .owner
        .pending_member_return
        .as_ref()
        .unwrap()
        .completed
        .is_none());
}
#[test]
fn signed_invalid_native_incarnation_refuses_before_pending_mutation() {
    for mutation in 0..4 {
        let mut f = Fixture::new("image-return-invalid-incarnation");
        let mut projection = f.projection.clone();
        match mutation {
            0 => projection.context.protocol_version += 1,
            1 => projection.context.offer_generation.0 = 0,
            2 => {
                projection.context.boot_id = "".into();
                projection.authority_grants.clear();
            }
            _ => {
                projection.context.profile =
                    "x".repeat(conduit_body::MAX_LIFECYCLE_ID_BYTES + 1).into()
            }
        }
        let signed = f.signed(projection);
        let before = f.owner.session.evidence().clone();
        assert!(f
            .owner
            .fixture_begin_image_member_return(signed, &f.images)
            .is_err());
        assert!(f.owner.pending_member_return.is_none());
        assert_eq!(f.owner.session.evidence(), &before);
    }
}

#[test]
fn production_requires_workspace_before_decode_or_core_for_image_and_unbounded_hosts() {
    for image_bound in [false, true] {
        let mut f = Fixture::new("image-return-no-workspace");
        if image_bound {
            let actual = f.owner.host.advertisement();
            let host = crate::durable_host_control::startup_member::test_fixture::selected_host(
                actual.host_id.as_str(),
                actual.boot_id.as_str(),
                false,
            );
            f.owner.host = crate::durable_host::owner::controller::OwnerHost::new(host).unwrap();
        }
        let before = f.owner.session.evidence().clone();
        let manager = f.owner.admissions.clone();
        let presentation = f.presentation();
        // Deliberately malformed bytes must never enter the image decoder.
        let error = f
            .owner
            .begin_image_member_return(presentation, &[vec![0]])
            .err()
            .unwrap();
        assert_eq!(error, super::super::WORKSPACE_REFUSAL);
        assert!(f.owner.pending_member_return.is_none());
        assert_eq!(f.owner.session.evidence(), &before);
        assert_eq!(f.owner.admissions, manager);
    }
}
#[test]
fn production_cannot_complete_fixture_pending_work_without_workspace() {
    let mut f = Fixture::new("image-return-no-completion-workspace");
    let challenge = f.begin().unwrap();
    let before = f.owner.session.evidence().clone();
    let proof = f.proof(&challenge);
    let error = f
        .owner
        .complete_image_member_return(&f.root, &proof)
        .err()
        .unwrap();
    assert_eq!(error, super::super::WORKSPACE_REFUSAL);
    assert!(f
        .owner
        .pending_member_return
        .as_ref()
        .unwrap()
        .completed
        .is_none());
    assert_eq!(f.owner.session.evidence(), &before);
    assert!(!f.root.join("body/member-return-receipts").exists());
}
