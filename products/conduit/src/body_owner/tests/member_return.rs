use super::member_leave::{admit_with_key, owner_fixture};
use super::*;
use conduit_body::{PartReturnChallenge, PartReturnProof, SpawnInvitationSecret};
fn prove(challenge: &PartReturnChallenge, key: &SpawnInvitationSecret) -> PartReturnProof {
    PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    }
}

#[test]
fn member_return_replaces_current_boot_only_after_proof_and_durable_commit() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, old) = admit_with_key(&mut owner, &root, "host/restarting");
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    let offers = owner.current_offer_summary();
    assert!(owner.begin_member_return(&prior, &old).is_err());
    let mut replacement = old.clone();
    replacement.boot_id = "boot/restarting/2".into();
    replacement.offer_generation.0 += 1;
    let challenge = owner.begin_member_return(&prior, &replacement).unwrap();
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    let proof = prove(&challenge, &key);
    let mut forged = proof.clone();
    forged.signature[0] ^= 1;
    assert!(owner.complete_member_return(&root, &forged).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    let blocked = root.join("replacement-commit-blocked");
    std::fs::write(&blocked, b"occupied").unwrap();
    assert!(owner.complete_member_return(&blocked, &proof).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    let returned = owner.complete_member_return(&root, &proof).unwrap();
    assert_eq!(returned.part_id, prior.part_id);
    assert_eq!(returned.host_id, prior.host_id);
    assert_eq!(returned.boot_id, replacement.boot_id);
    assert!(owner.require_observed_member_offer(&old).is_err());
    assert!(owner.require_observed_member_offer(&replacement).is_ok());
    let committed = owner.session.evidence().clone();
    assert_eq!(state::load(&root).unwrap().unwrap(), committed);
    assert_eq!(
        owner.complete_member_return(&root, &proof).unwrap(),
        returned
    );
    assert_eq!(owner.session.evidence(), &committed);
    assert!(owner.complete_member_return(&root, &forged).is_err());

    // Once another authenticated Boot replaces this one, neither the old
    // credential nor the preceding exact proof can displace the latest Boot.
    owner.pending_member_return = None;
    replacement.boot_id = "boot/restarting/3".into();
    replacement.offer_generation.0 += 1;
    assert!(owner.begin_member_return(&prior, &replacement).is_err());
    let challenge = owner.begin_member_return(&returned, &replacement).unwrap();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    let latest = owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .unwrap();
    assert_eq!(latest.part_id, prior.part_id);
    assert_eq!(latest.boot_id, replacement.boot_id);
    let committed = owner.session.evidence().clone();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    assert_eq!(owner.session.evidence(), &committed);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn member_return_replacement_refuses_changed_attachment_and_foreign_host() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut replacement) = admit_with_key(&mut owner, &root, "host/replacement-race");
    let before = owner.session.evidence().clone();
    replacement.boot_id = "boot/replacement-race/2".into();
    let mut foreign = replacement.clone();
    foreign.host_id = "host/foreign-replacement".into();
    assert!(owner.begin_member_return(&prior, &foreign).is_err());
    assert_eq!(owner.session.evidence(), &before);
    let challenge = owner.begin_member_return(&prior, &replacement).unwrap();
    // A distinct authorized withdrawal is not the condition accepted when
    // preparing this replacement. It needs a new finite return challenge.
    owner.member_leave(&root, &prior).unwrap();
    let withdrawn = owner.session.evidence().clone();
    assert!(owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .is_err());
    assert_eq!(owner.session.evidence(), &withdrawn);
    assert_eq!(state::load(&root).unwrap().unwrap(), withdrawn);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn current_member_offers_follow_authenticated_commit_not_membership_alone() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, old) = admit_with_key(&mut owner, &root, "host/equipment");
    assert!(owner.require_observed_member_offer(&old).is_ok());
    assert_eq!(
        owner.truth()["current_member_offers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let observed = owner.current_host_offers.clone();
    owner.member_leave(&root, &prior).unwrap();
    // A retained observation cannot become current by skipping reconciliation.
    assert_eq!(observed.current_hosts(owner.session.evidence()).count(), 0);
    assert!(owner.require_observed_member_offer(&old).is_err());
    let mut new = old.clone();
    new.boot_id = "boot/equipment/2".into();
    new.offer_generation.0 += 1;
    let challenge = owner.begin_member_return(&prior, &new).unwrap();
    let valid = prove(&challenge, &key);
    let mut forged = valid.clone();
    forged.signature[0] ^= 1;
    assert!(owner.complete_member_return(&root, &forged).is_err());
    assert!(owner.current_offer_summary().is_empty());
    let blocked = root.join("offer-commit-blocked");
    std::fs::write(&blocked, b"occupied").unwrap();
    assert!(owner.complete_member_return(&blocked, &valid).is_err());
    assert!(owner.current_offer_summary().is_empty());
    owner.complete_member_return(&root, &valid).unwrap();
    assert!(owner.require_observed_member_offer(&new).is_ok());
    assert!(owner.require_observed_member_offer(&old).is_err());
    let mut altered = new.clone();
    altered.capabilities.clear();
    assert_ne!(new, altered);
    assert!(owner.require_observed_member_offer(&altered).is_err());
    assert!(owner.session.realization().is_none());
    assert!(!owner.host.is_playing());
    // Opening retained membership does not resurrect equipment observations.
    let reopened = Owner::open(
        host("boot/member-offers-owner/2"),
        resident(&source()),
        Some(state::load(&root).unwrap().unwrap()),
        "Shared",
    )
    .unwrap();
    assert!(reopened.current_host_offers.hosts().is_empty());
    assert_eq!(
        reopened
            .current_host_offers
            .current_grants(reopened.session.evidence())
            .count(),
        0
    );
    assert!(reopened.current_offer_summary().is_empty());
    assert!(reopened.require_observed_member_offer(&new).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn member_return_commits_once_and_replays_only_the_exact_signed_outcome() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut offer) = admit_with_key(&mut owner, &root, "host/returning");
    assert!(owner.begin_member_return(&prior, &offer).is_err());
    owner.member_leave(&root, &prior).unwrap();
    offer.boot_id = "boot/returning/2".into();
    offer.offer_generation.0 += 1;
    let challenge = owner.begin_member_return(&prior, &offer).unwrap();
    assert_eq!(
        owner.begin_member_return(&prior, &offer).unwrap(),
        challenge
    );
    let proof = prove(&challenge, &key);
    let before = owner.session.evidence().clone();
    let mut forged = proof.clone();
    forged.signature[0] ^= 1;
    assert!(owner.complete_member_return(&root, &forged).is_err());
    assert_eq!(owner.session.evidence(), &before);
    let returned = owner.complete_member_return(&root, &proof).unwrap();
    assert_eq!(returned.part_id, prior.part_id);
    assert_eq!(returned.host_id, prior.host_id);
    assert_eq!(returned.boot_id, offer.boot_id);
    assert_ne!(returned.credential_id, prior.credential_id);
    let committed = owner.session.evidence().clone();
    assert_eq!(
        owner.complete_member_return(&root, &proof).unwrap(),
        returned
    );
    assert!(owner.complete_member_return(&root, &forged).is_err());
    assert_eq!(owner.session.evidence(), &committed);
    assert_eq!(state::load(&root).unwrap().unwrap(), committed);
    // A committed reply is not current admission after explicit withdrawal.
    owner.member_leave(&root, &returned).unwrap();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    assert!(owner.begin_member_return(&prior, &offer).is_err());
    // Process restart must not resurrect the ephemeral challenge.
    owner.pending_member_return = None;
    assert!(owner.complete_member_return(&root, &proof).is_err());
    assert!(owner.begin_member_return(&prior, &offer).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn member_return_persistence_failure_neither_attaches_nor_delivers_a_receipt() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, offer) = admit_with_key(&mut owner, &root, "host/return-fault");
    owner.member_leave(&root, &prior).unwrap();
    let challenge = owner.begin_member_return(&prior, &offer).unwrap();
    let proof = prove(&challenge, &key);
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    // A regular file cannot be a retained-state root.
    let blocked = root.join("blocked-root");
    std::fs::write(&blocked, b"occupied").unwrap();
    assert!(owner.complete_member_return(&blocked, &proof).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    owner.complete_member_return(&root, &proof).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn member_return_bad_proofs_exhaust_the_same_finite_challenge() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, offer) = admit_with_key(&mut owner, &root, "host/return-attempts");
    owner.member_leave(&root, &prior).unwrap();
    let challenge = owner.begin_member_return(&prior, &offer).unwrap();
    let valid = prove(&challenge, &key);
    let mut forged = valid.clone();
    forged.signature[0] ^= 1;
    let before = owner.session.evidence().clone();
    for _ in 0..conduit_body::MAX_ADMISSION_ATTEMPTS {
        assert!(owner.complete_member_return(&root, &forged).is_err());
    }
    assert!(owner.complete_member_return(&root, &valid).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(
        owner.begin_member_return(&prior, &offer).unwrap(),
        challenge
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn signed_return_acknowledgement_requires_committed_current_part_and_observed_offers() {
    use crate::durable_host::owner::MembershipReceiptDocument;
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut ad) = admit_with_key(&mut owner, &root, "host/return-receipt");
    owner.member_leave(&root, &prior).unwrap();
    ad.boot_id = "boot/return-receipt/2".into();
    ad.offer_generation.0 += 1;
    // A finite MCU presentation, rather than the full hosted fixture's catalog.
    // Receipt transport bounds stay unchanged.
    ad.bases.clear();
    ad.resources.clear();
    ad.capabilities.clear();
    ad.planner_capabilities.clear();
    let peer = owner.prepare_current_member_carrier_receiver().unwrap();
    let challenge = owner.begin_member_return(&prior, &ad).unwrap();
    let mut uncommitted = prior.clone();
    uncommitted.boot_id = ad.boot_id.clone();
    let uncommitted = MembershipReceiptDocument::Return(
        conduit_body::MemberReturnAcknowledgement::new(uncommitted, ad.clone()),
    );
    assert!(owner
        .attest_current_member_receipt(&uncommitted, &peer)
        .is_err());
    let current = owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .unwrap();
    let ack = conduit_body::MemberReturnAcknowledgement::new(current.clone(), ad.clone());
    let before = owner.session.evidence().clone();
    let signed = owner
        .attest_current_member_receipt(&MembershipReceiptDocument::Return(ack.clone()), &peer)
        .unwrap();
    let received = signed
        .verify_return(&peer, &key.verifying_key(), &prior, &ad)
        .unwrap();
    assert_eq!(received.credential, current);
    assert_eq!(received.credential.part_id, prior.part_id);
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    let mut changed = ack.clone();
    changed.advertisement.profile = "profile/unobserved".into();
    assert!(owner
        .attest_current_member_receipt(&MembershipReceiptDocument::Return(changed), &peer)
        .is_err());
    owner.member_leave(&root, &current).unwrap();
    assert!(owner
        .attest_current_member_receipt(&MembershipReceiptDocument::Return(ack), &peer)
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn image_return_attestation_resolves_exact_committed_offers_inside_owner() {
    use crate::durable_host::owner::MembershipReceiptDocument as Document;
    use conduit_body::{
        HostAdvertisementImageProjection as Projection, MemberReturnImageAcknowledgement as Ack,
    };
    use conduit_core as c;
    use sha2::{Digest, Sha256};
    fn image(offer: &c::CapabilityOffer) -> Vec<u8> {
        let json = serde_json::to_vec(offer).unwrap();
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&json, 6);
        let doc = conduit_wire::DeflatedDocument::from_parts(&compressed, json.len()).unwrap();
        let mut image = vec![0; 8];
        doc.write_header(&mut image).unwrap();
        image.extend(compressed);
        image
    }
    let mut offer = c::capability_offer_from_parts! {
        semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
        capability_id: "fixture/offer".into(), kind_id: "fixture/kind".into(), kind_contract_revision: "fixture/kind@1".into(),
        inputs: vec![], outputs: vec![],
        implementation: c::ImplementationOffer { execution_profile_id: "fixture/profile".into(), implementation_id: "fixture/back".into(), artifact_id: "fixture/artifact".into() },
        host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
        limits: c::CapabilityLimits { max_active_instances: 1,max_queue_items: 1,max_queue_bytes: 1 },
    };
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut ad) = admit_with_key(&mut owner, &root, "host/image-return");
    owner.member_leave(&root, &prior).unwrap();
    ad.boot_id = "boot/image-return/2".into();
    ad.offer_generation.0 += 1;
    ad.bases.clear();
    ad.resources.clear();
    ad.planner_capabilities.clear();
    let grant = c::AuthorityGrant {
        grant_id: "fixture/original-image-grant".into(),
        contract_id: "fixture/authority".into(),
        host_call_contract_id: "fixture/call".into(),
        subject_kind: offer.kind_id.clone(),
        capability_id: offer.capability_id.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
    };
    offer.authority_requirements.push(c::AuthorityRequirement {
        contract_id: grant.contract_id.clone(),
        host_call_contract_id: grant.host_call_contract_id.clone(),
        subject_kind: grant.subject_kind.clone(),
    });
    ad.capabilities = vec![offer.clone()];
    let bytes = image(&offer);
    let mut context = ad.clone();
    context.capabilities.clear();
    let projection = Projection {
        context,
        capability_images: vec![Sha256::digest(&bytes).into()],
        authority_grants: vec![grant.clone()],
    };
    let peer = owner.prepare_current_member_carrier_receiver().unwrap();
    let presentation = conduit_body::MemberReturnImagePresentation {
        kind: "Advertise".into(),
        schema: conduit_body::MEMBER_RETURN_IMAGES_SCHEMA.into(),
        credential: prior.clone(),
        projection: projection.clone(),
        signature: key
            .sign(&conduit_body::member_return_image_transcript(&prior, &projection).unwrap())
            .into(),
    };
    assert_eq!(
        conduit_std_host::pico_spawn::resolve_member_offer_images(
            &projection,
            std::slice::from_ref(&bytes)
        )
        .unwrap(),
        ad
    );
    let challenge = owner
        .begin_member_image_return(&prior, &ad, Some(presentation))
        .unwrap();
    let mut uncommitted = prior.clone();
    uncommitted.boot_id = ad.boot_id.clone();
    assert!(owner
        .attest_current_member_receipt(
            &Document::ImageReturn {
                receipt: Ack::new(uncommitted, projection.clone()),
                images: vec![bytes.clone()],
            },
            &peer
        )
        .is_err());
    let current = owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .unwrap();
    let before = owner.session.evidence().clone();
    let ack = Ack::new(current.clone(), projection.clone());
    let signed = owner
        .attest_current_member_receipt(
            &Document::ImageReturn {
                receipt: ack.clone(),
                images: vec![bytes.clone()],
            },
            &peer,
        )
        .unwrap();
    let receipt = signed
        .verify_image_return(&peer, &key.verifying_key(), &prior, &projection)
        .unwrap();
    assert_eq!(receipt.credential, current);
    let mut unaccepted = ack.clone();
    unaccepted.projection.authority_grants[0].grant_id = "fixture/unaccepted-grant".into();
    unaccepted.projection.validate().unwrap();
    assert!(owner
        .attest_current_member_receipt(
            &Document::ImageReturn {
                receipt: unaccepted,
                images: vec![bytes.clone()],
            },
            &peer
        )
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    let mut corrupted = bytes.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    assert!(owner
        .attest_current_member_receipt(
            &Document::ImageReturn {
                receipt: ack.clone(),
                images: vec![corrupted],
            },
            &peer
        )
        .is_err());
    // A valid replacement image/reference still cannot alter the committed offer.
    let mut foreign = offer;
    foreign.limits.max_queue_bytes += 1;
    let foreign = image(&foreign);
    let mut changed = ack.clone();
    changed.projection.capability_images[0] = Sha256::digest(&foreign).into();
    assert!(owner
        .attest_current_member_receipt(
            &Document::ImageReturn {
                receipt: changed,
                images: vec![foreign],
            },
            &peer
        )
        .is_err());
    assert!(owner
        .attest_current_member_receipt(
            &Document::ImageReturn {
                receipt: ack.clone(),
                images: vec![],
            },
            &peer
        )
        .is_err());
    owner.member_leave(&root, &current).unwrap();
    assert!(owner
        .attest_current_member_receipt(
            &Document::ImageReturn {
                receipt: ack,
                images: vec![bytes],
            },
            &peer
        )
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn original_member_grants_publish_only_after_signed_fresh_durable_return() {
    use conduit_body::{
        member_return_image_transcript, HostAdvertisementImageProjection,
        MemberReturnImagePresentation, MEMBER_RETURN_IMAGES_SCHEMA,
    };
    use conduit_core::{AuthorityGrant, AuthorityRequirement};
    for fault in ["none", "signature", "capability", "requirement", "boot"] {
        let (mut owner, root) = owner_fixture();
        let (prior, key, mut ad) = admit_with_key(&mut owner, &root, "host/grant-return");
        owner.member_leave(&root, &prior).unwrap();
        ad.boot_id = "boot/grant-return/2".into();
        ad.offer_generation.0 += 1;
        let offer = ad.capabilities.first_mut().unwrap();
        let requirement = AuthorityRequirement {
            contract_id: "fixture/authority".into(),
            host_call_contract_id: "fixture/call".into(),
            subject_kind: offer.kind_id.clone(),
        };
        offer.authority_requirements = vec![requirement.clone()];
        let mut grant = AuthorityGrant {
            grant_id: "fixture/original-grant".into(),
            contract_id: requirement.contract_id,
            host_call_contract_id: requirement.host_call_contract_id,
            subject_kind: requirement.subject_kind,
            host_id: ad.host_id.clone(),
            boot_id: ad.boot_id.clone(),
            capability_id: offer.capability_id.clone(),
        };
        match fault {
            "capability" => grant.capability_id = "fixture/absent".into(),
            "requirement" => grant.contract_id = "fixture/unselected".into(),
            "boot" => grant.boot_id = "fixture/old-boot".into(),
            _ => {}
        }
        let mut context = ad.clone();
        context.capabilities.clear();
        let projection = HostAdvertisementImageProjection {
            context,
            capability_images: vec![],
            authority_grants: vec![grant.clone()],
        };
        if fault == "boot" {
            assert!(projection.validate().is_err());
            continue;
        }
        let mut presentation = MemberReturnImagePresentation {
            kind: "Advertise".into(),
            schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
            credential: prior.clone(),
            signature: key
                .sign(&member_return_image_transcript(&prior, &projection).unwrap())
                .into(),
            projection,
        };
        if fault == "signature" {
            presentation.signature[0] ^= 1;
        }
        let challenge = owner
            .begin_member_image_return(&prior, &ad, Some(presentation))
            .unwrap();
        assert_eq!(
            owner
                .current_host_offers
                .current_grants(owner.session.evidence())
                .count(),
            0
        );
        let proof = prove(&challenge, &key);
        let before = owner.session.evidence().clone();
        let blocked = root.join("blocked-grant-commit");
        std::fs::write(&blocked, b"occupied").unwrap();
        assert!(owner.complete_member_return(&blocked, &proof).is_err());
        assert_eq!(owner.session.evidence(), &before);
        assert_eq!(
            owner
                .current_host_offers
                .current_grants(owner.session.evidence())
                .count(),
            0
        );
        if fault != "none" {
            assert!(owner.complete_member_return(&root, &proof).is_err());
            assert_eq!(owner.session.evidence(), &before);
            assert_eq!(
                owner
                    .current_host_offers
                    .current_grants(owner.session.evidence())
                    .count(),
                0
            );
            continue;
        }
        let current = owner.complete_member_return(&root, &proof).unwrap();
        assert_eq!(
            owner
                .current_host_offers
                .current_grants(owner.session.evidence())
                .collect::<Vec<_>>(),
            vec![&grant]
        );
        owner.complete_member_return(&root, &proof).unwrap();
        let mut replaced = owner.current_host_offers.clone();
        // An ordinary observation cannot inherit a preceding signed grant set.
        replaced.observe(owner.session.evidence(), ad).unwrap();
        assert_eq!(replaced.current_grants(owner.session.evidence()).count(), 0);
        let retained = owner.current_host_offers.clone();
        owner.member_leave(&root, &current).unwrap();
        assert_eq!(retained.current_grants(owner.session.evidence()).count(), 0);
        assert_eq!(
            owner
                .current_host_offers
                .current_grants(owner.session.evidence())
                .count(),
            0
        );
    }
}
