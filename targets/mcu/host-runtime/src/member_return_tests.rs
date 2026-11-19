//! Hosted protocol fixtures, not hardware or durable Core membership evidence.
use crate::{
    member_return::{Refusal, RetainedReturn, ReturnBuffers},
    tests::ready,
    transport::{DocumentTransport, MessageTransport},
};
use conduit_body::{MemberBootstrap, MembershipCredential};
use conduit_core::{AuthorityGrant, HostAdvertisement};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{vec, vec::Vec};
#[path = "member_return_tests/owner.rs"]
mod owner;
use owner::{Fault, Fixture, Owner};
const IMAGES: &[&[u8]] = &[b"fixture/image-one/opaque", b"fixture/image-two/opaque"];
#[test]
fn retained_prior_part_returns_to_actual_boot_with_original_keys_and_exact_images() {
    let fixture = Fixture::new();
    let returning = fixture.returning();
    let mut owner = Owner::new(&fixture, IMAGES, Fault::None);
    let mut hashes = [[0; 32]; 2];
    let mut input = [0; 4096];
    let mut output = [0; 1024];
    let accepted = ready(returning.exchange(
        &mut owner,
        IMAGES,
        &fixture.grants,
        ReturnBuffers {
            image_hashes: &mut hashes,
            input: &mut input,
            output: &mut output,
        },
    ))
    .unwrap();
    assert!(owner.proof_verified);
    assert_eq!(accepted.credential.body_id, fixture.retained.body_id);
    assert_eq!(accepted.credential.part_id, fixture.retained.part_id);
    assert_eq!(accepted.credential.host_id, fixture.actual.host_id);
    assert_eq!(accepted.credential.boot_id, fixture.actual.boot_id);
    assert_ne!(accepted.credential.boot_id, fixture.retained.boot_id);
    assert_ne!(
        accepted.credential.credential_id,
        fixture.retained.credential_id
    );
    for (hash, image) in hashes.iter().zip(IMAGES) {
        assert_eq!(*hash, <[u8; 32]>::from(Sha256::digest(image)));
    }
    assert!(input.iter().all(|b| *b == 0) && output.iter().all(|b| *b == 0));
}
#[test]
fn signed_wrong_owner_projection_part_or_boot_cannot_install_current_membership() {
    let fixture = Fixture::new();
    for (fault, wanted) in [
        (Fault::Owner, Refusal::Attestation),
        (Fault::Projection, Refusal::Receipt),
        (Fault::Part, Refusal::Receipt),
        (Fault::Boot, Refusal::Receipt),
        (Fault::ChallengePart, Refusal::Challenge),
        (Fault::ChallengeBoot, Refusal::Challenge),
    ] {
        let mut owner = Owner::new(&fixture, IMAGES, fault);
        let mut hashes = [[0; 32]; 2];
        let mut input = [0; 4096];
        let mut output = [0; 1024];
        assert!(
            matches!(ready(fixture.returning().exchange(&mut owner, IMAGES, &fixture.grants,
            ReturnBuffers { image_hashes: &mut hashes, input: &mut input, output: &mut output })), Err(error) if error == wanted)
        );
        assert_eq!(fixture.retained.boot_id.as_str(), "fixture/prior-boot");
    }
}
#[test]
fn lost_acknowledgement_preserves_retained_identity_and_fresh_return_can_recover() {
    let fixture = Fixture::new();
    let prior = fixture.retained.clone();
    for fault in [Fault::LostAck, Fault::None] {
        let mut owner = Owner::new(&fixture, IMAGES, fault);
        let mut hashes = [[0; 32]; 2];
        let mut input = [0; 4096];
        let mut output = [0; 1024];
        let result = ready(fixture.returning().exchange(
            &mut owner,
            IMAGES,
            &fixture.grants,
            ReturnBuffers {
                image_hashes: &mut hashes,
                input: &mut input,
                output: &mut output,
            },
        ));
        assert!(owner.proof_verified);
        if matches!(fault, Fault::LostAck) {
            assert!(matches!(result, Err(Refusal::Transport)));
        } else {
            assert_eq!(result.unwrap().credential.part_id, prior.part_id);
        }
        assert_eq!(fixture.retained, prior);
    }
}
#[derive(Default)]
struct NoEffects {
    effects: usize,
}
impl MessageTransport for NoEffects {
    type Error = ();
    async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        self.effects += 1;
        Err(())
    }
    async fn receive<'a>(&mut self, _: &'a mut [u8]) -> Result<&'a [u8], ()> {
        self.effects += 1;
        Err(())
    }
}
impl DocumentTransport for NoEffects {
    async fn receive_document(
        &mut self,
        _: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        self.effects += 1;
        Err(())
    }
}
fn refuses_before_io(
    fixture: &Fixture,
    images: &[&[u8]],
    hashes: &mut [[u8; 32]],
    expected: Refusal,
) {
    let mut link = NoEffects::default();
    let mut input = [0; 4096];
    let mut output = [0; 1024];
    assert!(
        matches!(ready(fixture.returning().exchange(&mut link, images, &fixture.grants,
        ReturnBuffers { image_hashes: hashes, input: &mut input, output: &mut output })), Err(error) if error == expected)
    );
    assert_eq!(link.effects, 0);
}
#[test]
fn immutable_inventory_and_caller_hash_bounds_refuse_before_carrier_effects() {
    let fixture = Fixture::new();
    refuses_before_io(&fixture, IMAGES, &mut [[0; 32]; 1], Refusal::Bounds);
    refuses_before_io(&fixture, &[b""], &mut [[0; 32]; 1], Refusal::Bounds);
    refuses_before_io(
        &fixture,
        &[IMAGES[0], IMAGES[0]],
        &mut [[0; 32]; 2],
        Refusal::Bounds,
    );
    let oversized = vec![1; 24 * 1024 + 1];
    refuses_before_io(&fixture, &[&oversized], &mut [[0; 32]; 1], Refusal::Bounds);
    let owned: Vec<Vec<u8>> = (0..33).map(|index| vec![index]).collect();
    let images: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
    refuses_before_io(&fixture, &images, &mut [[0; 32]; 33], Refusal::Bounds);
}
#[test]
fn stale_native_scope_or_unsupported_comparison_never_reaches_carrier() {
    for mutation in 0..8 {
        let mut fixture = Fixture::new();
        match mutation {
            0 => fixture.actual.protocol_version = 0,
            1 => fixture.actual.boot_id = "".into(),
            2 => fixture.actual.offer_generation.0 = 0,
            3 => fixture.actual.profile = "wrong/configuration".into(),
            4 => fixture.retained.host_id = "wrong/host".into(),
            5 => fixture.retained.part_id = serde_json::from_str(r#""wrong/part""#).unwrap(),
            6 => fixture.retained.body_id = serde_json::from_str(r#""wrong/body""#).unwrap(),
            _ => fixture.member = SigningKey::from_bytes(&[99; 32]),
        }
        refuses_before_io(&fixture, IMAGES, &mut [[0; 32]; 2], Refusal::Scope);
    }
    let mut fixture = Fixture::new();
    // A richer planner catalog is valid general Host data but unsupported by
    // this exact finite acknowledgement comparator; no owned fallback is used.
    fixture
        .actual
        .planner_capabilities
        .push(conduit_core::PlannerCapabilityOffer {
            profile_id: "fixture/planner@1".into(),
            limits: conduit_core::PlannerLimits {
                maximum_host_advertisements: 1,
                maximum_gears: 1,
                maximum_connections: 1,
                maximum_authority_grants: 1,
                maximum_protected_resource_grants: 1,
                maximum_line_offers: 1,
            },
        });
    refuses_before_io(&fixture, IMAGES, &mut [[0; 32]; 2], Refusal::Scope);
}
