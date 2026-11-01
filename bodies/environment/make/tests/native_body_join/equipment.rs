//! Actual persisted continuity custody authenticates complete equipment without
//! changing the installed Body or bypassing the observer-only transport guard.
use super::*;
use conduit_body::{
    CurrentHostOffers, PendingCurrentHostOfferChallenge, SealedBodyLifecycleSession,
    SpawnInvitationSecret, VerifiedCurrentHostOffer,
};

fn equipment(observer: &c::HostAdvertisement) -> c::HostAdvertisement {
    let requirement = c::ResourceRequirement {
        class_id: "fixture/equipment-workspace".into(),
        units: 256,
        protected_role: None,
        compute: None,
        content: None,
    };
    let capability = c::BackOfferBuilder::new(
        c::Kind {
            startup_parameters: vec![],
            shorthand: None,
            kind_id: "fixture/equipment".into(),
            kind_contract_revision: "fixture/equipment@1".into(),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 256,
            },
        },
        c::Back {
            capability_id: "fixture/equipment-back".into(),
            execution_profile_id: "fixture/cpu".into(),
            implementation_id: "fixture/equipment-implementation".into(),
            artifact_id: "fixture/equipment-artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![requirement],
            authority_requirements: vec![],
        },
    )
    .build();
    c::HostAdvertisement {
        bases: vec![c::BaseProviderAdvertisement {
            base_id: "fixture/equipment-base".into(),
            provider_instance_id: "fixture/provider".into(),
            provider_generation: 5,
            implementation_id: "fixture/provider-implementation".into(),
            mechanism_family: "fixture/equipment-family".into(),
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: c::BaseLifecycle::Ready,
            capability_ids: vec![capability.capability_id.clone()],
            resource_pool_ids: vec!["fixture/equipment-pool".into()],
        }],
        capabilities: vec![capability],
        resources: vec![c::ResourceOffer {
            pool_id: "fixture/equipment-pool".into(),
            class_id: "fixture/equipment-workspace".into(),
            capacity_units: 256,
            compute: None,
            content: None,
        }],
        ..observer.clone()
    }
}
fn pending(
    owner: &SealedBodyLifecycleSession,
    host: &c::HostAdvertisement,
    part: &conduit_body::PartId,
    nonce: u8,
) -> PendingCurrentHostOfferChallenge {
    PendingCurrentHostOfferChallenge::issue(
        owner.evidence(),
        host.host_id.clone(),
        host.boot_id.clone(),
        part.clone(),
        [nonce; 32],
        30,
        100,
    )
    .unwrap()
}
fn verify(
    owner: &SealedBodyLifecycleSession,
    host: &c::HostAdvertisement,
    pending: PendingCurrentHostOfferChallenge,
    advertisement: c::HostAdvertisement,
    signature: &[u8; 64],
) -> Result<VerifiedCurrentHostOffer, &'static str> {
    native_restart::with_admissions(|manager| {
        // The Store has moved out of its global slot, not exposed a key clone.
        assert!(!native_restart::publication_ready());
        pending
            .verify(
                manager,
                owner.evidence(),
                &host.host_id,
                &host.boot_id,
                advertisement,
                signature,
                31,
            )
            .map_err(|_| "equipment-proof")
    })
}
pub(super) fn prove(
    mut owner: SealedBodyLifecycleSession,
    root: &[u8; 32],
    host: &c::HostAdvertisement,
    signer: &SigningKey,
    original_request: &conduit_body::PortableSpawnAdmissionRequest,
) -> SealedBodyLifecycleSession {
    // A fresh operator-issued worker invitation establishes its own Part and
    // continuity key. The existing observer is never promoted or relabelled.
    let secret = SpawnInvitationSecret::from_csprng_bytes([171; 32]).unwrap();
    let claim = native_body_join::issue(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        secret.clone(),
        [172; 32],
        20,
        100,
    )
    .unwrap()
    .invitation
    .claim;
    let mut request = conduit_body::PortableMembershipAdmissionRequest {
        schema: conduit_body::MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: c::PROTOCOL_VERSION,
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_id: "host/equipment-worker".into(),
        boot_id: "boot/equipment-worker".into(),
        offer_generation: c::OfferGeneration(7),
        nonce: claim.nonce,
        signature: [0; 64],
    };
    request.signature = secret.sign(&claim.signing_transcript(
        &request.host_id,
        &request.boot_id,
        request.offer_generation,
    ));
    let draft = native_body_join::membership::admit(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        &request,
        21,
    )
    .unwrap();
    let signature = signer.sign(&draft.transcript()).to_bytes();
    let admitted = draft.signed(signature);
    admitted.verify(root, &request).unwrap();
    assert!(!admitted.current_offers_available && !admitted.plan_created && !admitted.play_created);
    let worker = c::HostAdvertisement {
        host_id: request.host_id.clone(),
        boot_id: request.boot_id.clone(),
        offer_generation: request.offer_generation,
        ..host.clone()
    };
    let part = admitted.credential.part_id;
    // Authenticate only after the actual Store journal has been parked/restored.
    let mut boxed = Some(Box::new(owner));
    let parked = native_restart::park_box(&mut boxed, root, &host.host_id, &host.boot_id).unwrap();
    owner = *native_restart::restore_parked(parked, &host.host_id, &host.boot_id, root).unwrap();
    let ad = equipment(&worker);
    let before = owner.evidence().clone();
    let realization = owner.realization().cloned();
    let journal = native_restart::generation();
    let manager = native_restart::with_admissions(|m| Ok(serde_json::to_vec(m).unwrap())).unwrap();
    // Complete equipment is not smuggled through the observer entrance.
    let mut rich = original_request.clone();
    rich.host_advertisement = ad.clone();
    let encoded = serde_json::to_vec(&rich).unwrap();
    assert!(native_body_join::wire::decode(&encoded).is_err());
    assert!(
        native_body_join::admit(&mut owner, root, &host.host_id, &host.boot_id, &rich, 31).is_err()
    );
    assert_eq!(owner.evidence(), &before);

    let mut inventory = CurrentHostOffers::new();
    let proof = pending(&owner, host, &part, 161);
    let signature = secret.sign(&proof.challenge().signing_transcript(&ad).unwrap());
    let verified = verify(&owner, host, proof, ad.clone(), &signature).unwrap();
    assert!(native_restart::publication_ready());
    inventory
        .observe_verified(owner.evidence(), &host.host_id, &host.boot_id, verified, 31)
        .unwrap();
    assert_eq!(inventory.hosts(), core::slice::from_ref(&ad));
    let accepted = inventory.clone();

    // The persisted manager, not a reconstructed supplied-key verifier, refuses
    // changed equipment. A refusal cannot replace the previous complete offer.
    let proof = pending(&owner, host, &part, 162);
    let signature = secret.sign(&proof.challenge().signing_transcript(&ad).unwrap());
    let mut tampered = ad.clone();
    tampered.bases[0].provider_generation += 1;
    assert!(verify(&owner, host, proof, tampered, &signature).is_err());
    assert_eq!(inventory, accepted);

    let proof = pending(&owner, host, &part, 163);
    let signature = secret.sign(&proof.challenge().signing_transcript(&ad).unwrap());
    let mut stale = host.clone();
    stale.boot_id = "boot/root-after-restart".into();
    assert!(verify(&owner, &stale, proof, ad.clone(), &signature).is_err());
    assert_eq!(inventory, accepted);

    let proof = pending(&owner, host, &part, 164);
    let signature = secret.sign(&proof.challenge().signing_transcript(&ad).unwrap());
    let verified = verify(&owner, host, proof, ad.clone(), &signature).unwrap();
    let mut changed = owner.evidence().clone();
    changed.membership.revision.0 += 1;
    assert!(inventory
        .observe_verified(&changed, &host.host_id, &host.boot_id, verified, 31)
        .is_err());
    assert_eq!(inventory, accepted);

    assert_eq!(native_restart::generation(), journal);
    // A parked/unpublished Store cannot yield authenticated equipment. No
    // inventory write occurs before that actual custody gate has returned.
    let proof = pending(&owner, host, &part, 165);
    let signature = secret.sign(&proof.challenge().signing_transcript(&ad).unwrap());
    let mut boxed = Some(Box::new(owner));
    let parked = native_restart::park_box(&mut boxed, root, &host.host_id, &host.boot_id).unwrap();
    assert!(!native_restart::publication_ready());
    let mut entered = false;
    assert!(native_restart::with_admissions(|_| {
        entered = true;
        Ok(())
    })
    .is_err());
    assert!(!entered);
    assert_eq!(inventory, accepted);
    owner = *native_restart::restore_parked(parked, &host.host_id, &host.boot_id, root).unwrap();
    let verified = verify(&owner, host, proof, ad.clone(), &signature).unwrap();
    inventory
        .observe_verified(owner.evidence(), &host.host_id, &host.boot_id, verified, 31)
        .unwrap();
    assert_eq!(inventory, accepted);
    assert_eq!(owner.evidence(), &before);
    assert_eq!(owner.realization(), realization.as_ref());
    // Parking itself performs a real durable checkpoint; observations above did not.
    assert!(native_restart::generation().unwrap() > journal.unwrap());
    native_restart::with_admissions(|m| {
        assert_eq!(serde_json::to_vec(m).unwrap(), manager);
        Ok(())
    })
    .unwrap();
    owner
}
