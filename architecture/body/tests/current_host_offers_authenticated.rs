#![cfg(feature = "authenticated-admission")]
use conduit_body::*;
use conduit_core as c;
#[path = "current_host_offers_authenticated/support.rs"]
mod support;

struct Fixture {
    manager: AdmissionManager,
    evidence: BodyBiographyEvidence,
    secret: SpawnInvitationSecret,
    part: PartId,
    ad: c::HostAdvertisement,
}
fn fixture() -> Fixture {
    let body = Body::born(
        "source/offer".into(),
        "checked/offer".into(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes([71; 32]).unwrap();
    let invitation = manager
        .issue_spawn_invitation(secret.clone(), [72; 32], 1, 100)
        .unwrap();
    let ad = c::HostAdvertisement {
        declared_name: Some(c::HostDeclaredName::new("actual complete equipment fixture").unwrap()),
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "worker".into(),
        boot_id: "worker-boot".into(),
        offer_generation: c::OfferGeneration(3),
        profile: "fixture".into(),
        bases: vec![c::BaseProviderAdvertisement {
            base_id: "capture".into(),
            provider_instance_id: "capture-instance".into(),
            provider_generation: 1,
            implementation_id: "capture-provider".into(),
            mechanism_family: "fixture".into(),
            enforcement_class: c::BaseEnforcementClass::ProcessIsolated,
            lifecycle: c::BaseLifecycle::Ready,
            capability_ids: vec!["fixture/back".into()],
            resource_pool_ids: vec!["pcm".into()],
        }],
        capabilities: vec![support::capability(8)],
        planner_capabilities: vec![],
        resources: vec![c::ResourceOffer {
            pool_id: "pcm".into(),
            class_id: "memory".into(),
            capacity_units: 4096,
            compute: None,
            content: None,
        }],
    };
    let proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: body.body_id.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        nonce: invitation.nonce,
        signature: secret.sign(&invitation.signing_transcript(
            &ad.host_id,
            &ad.boot_id,
            ad.offer_generation,
        )),
    };
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body, membership.clone(), "equipment".into()).unwrap();
    let credential = manager
        .complete_spawn(
            &mut membership,
            &ad,
            &proof,
            2,
            AdmissionSigns {
                part_admitted: "part".into(),
                host_attached: "attached".into(),
                candidate_admitted: "candidate".into(),
            },
        )
        .unwrap();
    let events = membership
        .events
        .iter()
        .enumerate()
        .map(|(index, event)| (event.change_id.clone(), index as u64 + 2))
        .collect::<Vec<_>>();
    evidence
        .append_membership_events(membership, &events)
        .unwrap();
    Fixture {
        manager,
        evidence,
        secret,
        part: credential.part_id,
        ad,
    }
}
fn pending(f: &Fixture, nonce: u8) -> PendingCurrentHostOfferChallenge {
    PendingCurrentHostOfferChallenge::issue(
        &f.evidence,
        "root".into(),
        "root-boot".into(),
        f.part.clone(),
        [nonce; 32],
        10,
        100,
    )
    .unwrap()
}
fn signature(f: &Fixture, p: &PendingCurrentHostOfferChallenge) -> [u8; 64] {
    f.secret
        .sign(&p.challenge().signing_transcript(&f.ad).unwrap())
}
fn verify(
    f: &Fixture,
    p: PendingCurrentHostOfferChallenge,
    ad: c::HostAdvertisement,
    sig: &[u8; 64],
    now: u64,
) -> Result<VerifiedCurrentHostOffer, CurrentHostOfferAuthenticationError> {
    p.verify(
        &f.manager,
        &f.evidence,
        &"root".into(),
        &"root-boot".into(),
        ad,
        sig,
        now,
    )
}
#[test]
fn complete_equipment_moves_into_inventory_without_membership_or_execution_change() {
    let f = fixture();
    let before = f.evidence.clone();
    let p = pending(&f, 1);
    let sig = signature(&f, &p);
    let expected = f.ad.clone();
    let verified = verify(&f, p, f.ad.clone(), &sig, 100).unwrap();
    let mut offers = CurrentHostOffers::new();
    offers
        .observe_verified(
            &f.evidence,
            &"root".into(),
            &"root-boot".into(),
            verified,
            100,
        )
        .unwrap();
    assert_eq!(offers.hosts(), &[expected]);
    assert_eq!(f.evidence, before);
}
#[test]
fn descriptor_tampering_and_wrong_key_refuse_without_inventory_mutation() {
    let f = fixture();
    let mut offers = CurrentHostOffers::new();
    offers.observe(&f.evidence, f.ad.clone()).unwrap();
    let before = offers.clone();
    for field in 0..4 {
        let p = pending(&f, field + 1);
        let sig = signature(&f, &p);
        let mut ad = f.ad.clone();
        match field {
            0 => ad.resources[0].capacity_units += 1,
            1 => ad.declared_name = Some(c::HostDeclaredName::new("forged").unwrap()),
            2 => ad.resources[0].pool_id = "forged-pool".into(),
            _ => ad.profile = "another-profile".into(),
        }
        assert!(verify(&f, p, ad, &sig, 11).is_err());
        assert_eq!(offers, before);
    }
    let p = pending(&f, 9);
    let transcript = p.challenge().signing_transcript(&f.ad).unwrap();
    let wrong = SpawnInvitationSecret::from_csprng_bytes([99; 32])
        .unwrap()
        .sign(&transcript);
    assert!(verify(&f, p, f.ad.clone(), &wrong, 11).is_err());
    assert_eq!(offers, before);
}
#[test]
fn nonce_root_boot_scope_deadline_and_incarnation_are_bound() {
    let f = fixture();
    let old = pending(&f, 1);
    let sig = signature(&f, &old);
    assert!(verify(&f, pending(&f, 2), f.ad.clone(), &sig, 11).is_err());
    for field in 0..5 {
        let p = pending(&f, 3);
        let sig = signature(&f, &p);
        let mut ad = f.ad.clone();
        let mut root = c::HostId::from("root");
        let mut boot = c::BootId::from("root-boot");
        match field {
            0 => root = "wrong-root".into(),
            1 => boot = "later-root-boot".into(),
            2 => ad.host_id = "other".into(),
            3 => ad.boot_id = "later".into(),
            _ => ad.offer_generation.0 += 1,
        }
        assert!(p
            .verify(&f.manager, &f.evidence, &root, &boot, ad, &sig, 11)
            .is_err());
    }
    for now in [9, 101] {
        let p = pending(&f, 4);
        let sig = signature(&f, &p);
        assert!(verify(&f, p, f.ad.clone(), &sig, now).is_err());
    }
    // Public challenge serialization cannot reconstruct the affine pending owner.
    let public: CurrentHostOfferChallenge =
        serde_json::from_slice(&serde_json::to_vec(old.challenge()).unwrap()).unwrap();
    assert_eq!(&public, old.challenge());
}
#[test]
fn changed_membership_refuses_both_before_and_after_verification() {
    let mut f = fixture();
    let p = pending(&f, 1);
    let sig = signature(&f, &p);
    let verified = verify(&f, p, f.ad.clone(), &sig, 11).unwrap();
    let p = pending(&f, 2);
    let sig = signature(&f, &p);
    f.evidence.membership.revision.0 += 1;
    assert!(verify(&f, p, f.ad.clone(), &sig, 11).is_err());
    let mut offers = CurrentHostOffers::new();
    assert!(offers
        .observe_verified(
            &f.evidence,
            &"root".into(),
            &"root-boot".into(),
            verified,
            11
        )
        .is_err());
    assert!(offers.hosts().is_empty());
}
#[test]
fn issuance_requires_present_member_entropy_and_finite_deadline() {
    let f = fixture();
    for (nonce, now, end) in [([0; 32], 10, 100), ([1; 32], 10, 10), ([1; 32], 10, 30_011)] {
        assert!(PendingCurrentHostOfferChallenge::issue(
            &f.evidence,
            "root".into(),
            "root-boot".into(),
            f.part.clone(),
            nonce,
            now,
            end
        )
        .is_err());
    }
    assert!(PendingCurrentHostOfferChallenge::issue(
        &f.evidence,
        "root".into(),
        "root-boot".into(),
        PartId::bind(&f.evidence.body_id, "absent", 1).unwrap(),
        [1; 32],
        10,
        100
    )
    .is_err());
}

#[test]
fn immutable_defaults_and_base_runtime_evidence_are_authenticated() {
    let f = fixture();
    for field in 0..3 {
        let p = pending(&f, field + 1);
        let sig = signature(&f, &p);
        let mut ad = f.ad.clone();
        match field {
            0 => ad.capabilities[0] = support::capability(9),
            1 => ad.bases[0].provider_generation += 1,
            _ => ad.bases[0].lifecycle = c::BaseLifecycle::Lost,
        }
        assert!(verify(&f, p, ad, &sig, 11).is_err());
    }
}

#[path = "common/admission_allocation_probe.rs"]
mod allocation;
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;

#[test]
fn full_descriptor_transcript_streams_large_defaults_without_allocating() {
    let mut f = fixture();
    f.ad.capabilities[0] = support::capability(129_539);
    let pending = pending(&f, 10);
    let expected = pending.challenge().signing_transcript(&f.ad).unwrap();
    let guard = allocation::refuse_next_allocation_size(129_539);
    let (result, measured) = allocation::observe(|| pending.challenge().signing_transcript(&f.ad));
    drop(guard);
    assert_eq!(result.unwrap(), expected);
    assert_eq!(measured.allocations, 0, "{measured:?}");
    assert_eq!(measured.reallocations, 0, "{measured:?}");
    assert_eq!(measured.requested_bytes, 0, "{measured:?}");
    assert_eq!(measured.retained_bytes(), 0);
}

#[path = "../src/current_host_offers/descriptor_hash.rs"]
mod descriptor_hash;
#[test]
fn borrowed_complete_descriptor_matches_original_serde_with_optional_fields() {
    let f = fixture();
    for name in [false, true] {
        for bases in [false, true] {
            let mut ad = f.ad.clone();
            if !name {
                ad.declared_name = None;
            }
            if !bases {
                ad.bases.clear();
            }
            assert_eq!(
                descriptor_hash::advertisement_digest(&ad).unwrap(),
                descriptor_hash::digest(&ad).unwrap()
            );
        }
    }
}

#[test]
fn complete_offer_transcript_v1_is_stable() {
    let f = fixture();
    let p = pending(&f, 1);
    assert_eq!(
        p.challenge().signing_transcript(&f.ad).unwrap(),
        [
            220, 66, 96, 245, 241, 178, 206, 153, 146, 2, 101, 91, 58, 119, 210, 19, 254, 82, 98,
            191, 43, 172, 72, 172, 227, 36, 221, 222, 145, 168, 171, 131
        ]
    );
}
