//! Membership parity only: identity completion grants no equipment or Play.
use conduit_body::*;
use conduit_core as c;
#[allow(dead_code)]
#[path = "common/admission_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const NOW: u64 = 10_000;
const EXPIRES: u64 = 20_000;
struct Fixture {
    manager: AdmissionManager,
    membership: BodyMembership,
    advertisement: c::HostAdvertisement,
    proof: SpawnAdmissionProof,
}
fn fixture() -> Fixture {
    let body = Body::born(
        "source/scope".into(),
        "checked/scope".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let membership = BodyMembership::new(body.body_id).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    let advertisement = c::HostAdvertisement {
        declared_name: Some(c::HostDeclaredName::new("initialized worker fixture").unwrap()),
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/worker".into(),
        boot_id: "boot/worker".into(),
        offer_generation: c::OfferGeneration(3),
        profile: "profile/worker".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![capability()],
        planner_capabilities: vec![],
    };
    let proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        )),
    };
    Fixture {
        manager,
        membership,
        advertisement,
        proof,
    }
}
fn capability() -> c::CapabilityOffer {
    let ty = c::StructuredInfoType::leaf(c::kind_id(c::TEXT_INFO_ID)).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let value = c::StructuredInfoValue::leaf(ty, vec![b'x'; 129_539]).unwrap();
    let default =
        c::StructuredConfigurationValue::new(profile.clone(), value.canonical_bytes().unwrap())
            .unwrap();
    c::BackOfferBuilder::new(
        c::Kind {
            startup_parameters: vec![c::FrontStartupParameter {
                name: "default".into(),
                value_type: profile.clone(),
                has_default: true,
            }],
            shorthand: None,
            kind_id: "fixture/large-default".into(),
            kind_contract_revision: "fixture/revision".into(),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![c::KindConfigurationField {
                key: "default".into(),
                default_value: c::ConfigurationValue::Structured(default),
                rule: c::KindConfigurationRule::Structured { profile },
            }],
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        },
        c::Back {
            capability_id: "fixture/back".into(),
            execution_profile_id: "fixture/profile".into(),
            implementation_id: "fixture/implementation".into(),
            artifact_id: "fixture/artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
fn signs() -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: "sign/part".into(),
        host_attached: "sign/host".into(),
        candidate_admitted: "sign/candidate".into(),
    }
}
fn compare(f: &Fixture, now: u64) -> Result<MembershipCredential, AdmissionRefusal> {
    let mut full = f.manager.clone();
    let mut scoped = f.manager.clone();
    let mut full_membership = f.membership.clone();
    let mut scoped_membership = f.membership.clone();
    let scope = SpawnAdmissionHostScope::new(
        &f.advertisement.host_id,
        &f.advertisement.boot_id,
        f.advertisement.offer_generation,
    );
    let a = full.complete_spawn(
        &mut full_membership,
        &f.advertisement,
        &f.proof,
        now,
        signs(),
    );
    let b = scoped.complete_spawn_for_host(&mut scoped_membership, scope, &f.proof, now, signs());
    assert_eq!(a, b);
    assert_eq!(full_membership, scoped_membership);
    assert_eq!(
        serde_json::to_vec(&full).unwrap(),
        serde_json::to_vec(&scoped).unwrap()
    );
    if a.is_err() {
        assert_eq!(full_membership, f.membership);
    }
    a
}
#[test]
fn full_and_identity_only_completion_preserve_exact_membership_without_reading_equipment() {
    let f = fixture();
    let original = serde_json::to_vec(&f.advertisement).unwrap();
    assert!(original.len() > admission_advertisement_wire::MAXIMUM_EXPANDED_BYTES);
    let credential = compare(&f, NOW + 1).unwrap();
    assert_eq!(credential.host_id, f.advertisement.host_id);
    assert_eq!(credential.boot_id, f.advertisement.boot_id);
    assert_eq!(serde_json::to_vec(&f.advertisement).unwrap(), original);
}
#[test]
fn identity_only_completion_needs_no_advertisement_object() {
    let mut f = fixture();
    let Fixture { advertisement, .. } = &f;
    let host = advertisement.host_id.clone();
    let boot = advertisement.boot_id.clone();
    let generation = advertisement.offer_generation;
    drop(f.advertisement);
    let credential = f
        .manager
        .complete_spawn_for_host(
            &mut f.membership,
            SpawnAdmissionHostScope::new(&host, &boot, generation),
            &f.proof,
            NOW + 1,
            signs(),
        )
        .unwrap();
    assert_eq!(credential.host_id, host);
    assert_eq!(f.membership.parts.len(), 1);
    assert_eq!(
        f.membership.parts[0]
            .current
            .as_ref()
            .unwrap()
            .offer_generation,
        generation
    );
}
#[test]
fn exact_failure_order_matches_for_foreign_scope_nonce_signature_and_expiry() {
    let f = fixture();
    for (case, expected) in [
        (0, AdmissionRefusal::WrongBody),
        (1, AdmissionRefusal::WrongHost),
        (2, AdmissionRefusal::StaleBoot),
        (3, AdmissionRefusal::StaleNonce),
        (4, AdmissionRefusal::InvalidProof),
        (5, AdmissionRefusal::InvalidProof),
        (6, AdmissionRefusal::Expired),
        (7, AdmissionRefusal::UnknownInvitation),
    ] {
        let mut bad = Fixture {
            manager: f.manager.clone(),
            membership: f.membership.clone(),
            advertisement: f.advertisement.clone(),
            proof: f.proof.clone(),
        };
        match case {
            0 => {
                bad.proof.body_id = Body::born(
                    "other/source".into(),
                    "other/checked".into(),
                    2,
                    "other/sign".into(),
                )
                .unwrap()
                .body_id
            }
            1 => bad.advertisement.host_id = "host/foreign".into(),
            2 => bad.advertisement.boot_id = "boot/foreign".into(),
            3 => bad.proof.nonce[0] ^= 1,
            4 => bad.proof.signature[0] ^= 1,
            5 => bad.advertisement.offer_generation = c::OfferGeneration(4),
            6 => {}
            7 => bad.proof.invitation_id = serde_json::from_str("\"foreign-invitation\"").unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            compare(&bad, if case == 6 { EXPIRES + 1 } else { NOW + 1 }),
            Err(expected)
        );
    }
}
#[test]
fn signature_attempt_exhaustion_and_successful_replay_are_identical() {
    let f = fixture();
    let mut full = f.manager.clone();
    let mut scoped = f.manager.clone();
    let mut a = f.membership.clone();
    let mut b = f.membership.clone();
    let mut bad = f.proof.clone();
    bad.signature[0] ^= 1;
    for _ in 0..=MAX_ADMISSION_ATTEMPTS {
        let x = full.complete_spawn(&mut a, &f.advertisement, &bad, NOW + 1, signs());
        let y = scoped.complete_spawn_for_host(
            &mut b,
            SpawnAdmissionHostScope::from(&f.advertisement),
            &bad,
            NOW + 1,
            signs(),
        );
        assert_eq!(x, y);
        assert_eq!(a, f.membership);
        assert_eq!(b, f.membership);
        assert_eq!(
            serde_json::to_vec(&full).unwrap(),
            serde_json::to_vec(&scoped).unwrap()
        );
    }
    assert_eq!(
        full.complete_spawn(&mut a, &f.advertisement, &f.proof, NOW + 1, signs()),
        Err(AdmissionRefusal::AttemptsExhausted)
    );
    let mut used = fixture();
    used.manager
        .complete_spawn_for_host(
            &mut used.membership,
            SpawnAdmissionHostScope::from(&used.advertisement),
            &used.proof,
            NOW + 1,
            signs(),
        )
        .unwrap();
    assert_eq!(compare(&used, NOW + 2), Err(AdmissionRefusal::Replay));
}
#[test]
fn late_membership_refusal_and_receipt_capacity_preserve_atomicity_and_invitation() {
    let mut foreign = fixture();
    let other = Body::born(
        "other/source".into(),
        "other/checked".into(),
        2,
        "other/sign".into(),
    )
    .unwrap();
    foreign.membership = BodyMembership::new(other.body_id).unwrap();
    assert_eq!(compare(&foreign, NOW + 1), Err(AdmissionRefusal::WrongBody));
    let mut full = fixture();
    let mut one = full.manager.clone();
    let mut membership = full.membership.clone();
    one.complete_spawn(
        &mut membership,
        &full.advertisement,
        &full.proof,
        NOW + 1,
        signs(),
    )
    .unwrap();
    full.manager.receipts = vec![one.receipts[0].clone(); MAX_ADMISSION_RECEIPTS];
    assert_eq!(
        compare(&full, NOW + 1),
        Err(AdmissionRefusal::ReceiptCapacityExhausted)
    );
}

fn next_signed_admission(f: &mut Fixture, sequence: u8) {
    let invitation = f
        .manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([sequence; 32]).unwrap(),
            [sequence + 1; 32],
            NOW,
            EXPIRES,
        )
        .unwrap();
    f.advertisement.host_id = format!("host/worker/{sequence}").into();
    f.advertisement.boot_id = format!("boot/worker/{sequence}").into();
    f.proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: f.advertisement.host_id.clone(),
        boot_id: f.advertisement.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &f.advertisement.host_id,
            &f.advertisement.boot_id,
            f.advertisement.offer_generation,
        )),
    };
}
fn complete(f: &mut Fixture) -> Result<MembershipCredential, AdmissionRefusal> {
    let sequence = f.manager.receipts.len();
    f.manager.complete_spawn_for_host(
        &mut f.membership,
        SpawnAdmissionHostScope::from(&f.advertisement),
        &f.proof,
        NOW + 1,
        AdmissionSigns {
            part_admitted: format!("sign/part/{sequence}").into(),
            host_attached: format!("sign/host/{sequence}").into(),
            candidate_admitted: format!("sign/candidate/{sequence}").into(),
        },
    )
}

#[test]
fn signed_admissions_retain_exact_receipt_capacity_instead_of_geometric_growth() {
    let mut f = fixture();
    for sequence in 1..=8 {
        if sequence != 1 {
            next_signed_admission(&mut f, sequence);
        }
        complete(&mut f).unwrap();
        assert_eq!(f.manager.receipts.len(), sequence as usize);
        assert_eq!(f.manager.receipts.capacity(), sequence as usize);
    }
}
#[test]
fn exact_receipt_allocation_refusal_preserves_membership_authority_and_invitation() {
    let mut f = fixture();
    for sequence in 1..=8 {
        if sequence != 1 {
            next_signed_admission(&mut f, sequence);
        }
        complete(&mut f).unwrap();
    }
    next_signed_admission(&mut f, 9);
    let before = serde_json::to_vec(&f.manager).unwrap();
    let membership = f.membership.clone();
    let pointer = f.manager.receipts.as_ptr();
    let required = (f.manager.receipts.len() + 1) * core::mem::size_of_val(&f.manager.receipts[0]);
    let refusal = allocation_probe::refuse_next_reallocation(pointer.cast(), required);
    let result = complete(&mut f);
    drop(refusal);
    assert_eq!(result, Err(AdmissionRefusal::ReceiptCapacityExhausted));
    assert_eq!(f.membership, membership);
    assert_eq!(serde_json::to_vec(&f.manager).unwrap(), before);
    assert_eq!(f.manager.receipts.as_ptr(), pointer);
    assert_eq!(f.manager.receipts.capacity(), 8);
    let credential = complete(&mut f).unwrap();
    assert_eq!(credential.host_id, f.advertisement.host_id);
    assert_eq!(f.manager.receipts.len(), 9);
    assert_eq!(f.manager.receipts.capacity(), 9);
    assert_eq!(complete(&mut f), Err(AdmissionRefusal::Replay));
}
