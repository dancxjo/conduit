//! Hosted signed-protocol fixtures; no product transaction or durable commit.
use super::*;
use crate::{
    tests::ready,
    transport::{DocumentTransport, MessageTransport},
};
use conduit_body::{
    attest_member_return_image_acknowledgement, member_return_image_transcript,
    HostAdvertisementImageProjection, MemberReturnImageAcknowledgement,
    MemberReturnImagePresentation, MemberReturnProofDocument, MembershipCredential,
    PartReturnChallenge, PartReturnProof, SignedPortableAdmissionReceipt,
    MEMBER_RETURN_IMAGES_SCHEMA,
};
use conduit_core::{CapabilityOffer, HostAdvertisement};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, vec, vec::Vec};
#[path = "tests/fixtures.rs"]
mod fixtures;
use fixtures::{Fixture, Link};
fn limits() -> Limits {
    Limits {
        maximum_presentation_bytes: 4096,
        maximum_document_chunks: 64,
        maximum_images: 4,
        maximum_image_bytes: 4096,
        maximum_total_image_bytes: 8192,
        maximum_expanded_offer_bytes: 8192,
        maximum_total_expanded_bytes: 16384,
    }
}
fn observe_fixture(f: &Fixture, link: &mut Link<'_>) -> Result<Observation, Refusal> {
    let mut presentation = [0; 4096];
    let mut images = [0; 8192];
    let mut decode = [0; 8192];
    ready(observe(
        link,
        f.basis(),
        limits(),
        ReceiveBuffers {
            presentation: &mut presentation,
            images: &mut images,
            decode: &mut decode,
        },
    ))
}
#[test]
fn original_signature_images_proof_and_attested_receipt_use_one_generic_transport() {
    let f = Fixture::new();
    let mut link = f.link();
    let observation = observe_fixture(&f, &mut link).unwrap();
    assert_eq!(link.received_documents, 2);
    assert_eq!(observation.credential(), &f.prior);
    assert_eq!(
        observation.advertisement().capabilities,
        vec![f.offer.clone()]
    );
    assert_eq!(observation.advertisement().boot_id, f.actual.boot_id);
    let digest: [u8; 32] = Sha256::digest(&f.image).into();
    assert_eq!(observation.projection().capability_images, vec![digest]);
    let challenge = f.challenge();
    let mut frame = [0; 2048];
    let proof = ready(prove(
        &mut link,
        f.basis(),
        &observation,
        &challenge,
        1001,
        &mut frame,
    ))
    .unwrap();
    assert_eq!(proof.admission_id, challenge.admission_id);
    assert_eq!(proof.part_id, f.prior.part_id);
    assert_eq!(proof.boot_id, f.actual.boot_id);
    f.member
        .verifying_key()
        .verify_strict(
            &challenge.signing_transcript(),
            &ed25519_dalek::Signature::from_bytes(&proof.signature),
        )
        .unwrap();
    // The fixture signs a normal receipt only. The real product must bind this
    // whole projection to its existing pending transaction and complete Core
    // durability BEFORE it calls its attester/deliver; no such claim is made here.
    let signed = f.receipt(observation.projection().clone());
    let accepted = ready(deliver(
        &mut link,
        f.basis(),
        &observation,
        &signed,
        &mut frame,
    ))
    .unwrap();
    assert_eq!(accepted.part_id, f.prior.part_id);
    assert_eq!(accepted.boot_id, f.actual.boot_id);
    assert_ne!(accepted.boot_id, f.prior.boot_id);
    let metadata: conduit_body::BorrowedAdmissionReceiptAttestation<'_> =
        serde_json::from_slice(&link.sent[1]).unwrap();
    metadata
        .verify(
            signed.receipt_json.as_bytes(),
            &f.bootstrap.owner,
            &f.member.verifying_key().to_bytes(),
        )
        .unwrap();
    let mut received = conduit_wire::BoundedDocumentAssembly::new(16384, 64);
    for frame in &link.sent[2..] {
        received.push(frame).unwrap();
    }
    assert_eq!(
        received.completed().unwrap().0,
        signed.receipt_json.as_bytes()
    );
}
#[test]
fn invalid_presentation_is_rejected_before_any_image_read_or_decode() {
    for mutation in 0..5 {
        let f = Fixture::new();
        let mut presentation = f.presentation();
        match mutation {
            0 => presentation.signature[0] ^= 1,
            1 => {
                presentation.credential.credential_id =
                    serde_json::from_str(r#""wrong/credential""#).unwrap()
            }
            2 => presentation.projection.context.profile = "wrong/profile".into(),
            3 => presentation.projection.context.planner_capabilities.push(
                conduit_core::PlannerCapabilityOffer {
                    profile_id: "fixture/planner@1".into(),
                    limits: conduit_core::PlannerLimits {
                        maximum_host_advertisements: 1,
                        maximum_gears: 1,
                        maximum_connections: 1,
                        maximum_authority_grants: 1,
                        maximum_protected_resource_grants: 1,
                        maximum_line_offers: 1,
                    },
                },
            ),
            _ => presentation
                .projection
                .capability_images
                .push(presentation.projection.capability_images[0]),
        }
        // Except the deliberately bad signature, sign the changed whole
        // projection so exact-basis/profile/unsupported checks are exercised.
        if mutation != 0 {
            // Duplicate hashes violate the projection law before signing; use
            // the original signature there and require any strict refusal.
            if mutation != 4 {
                presentation.signature = f.sign_presentation(&presentation);
            }
        }
        let mut link = f.link();
        link.documents[0] = serde_json::to_vec(&presentation).unwrap();
        assert!(observe_fixture(&f, &mut link).is_err());
        assert_eq!(link.received_documents, 1);
        assert!(link.sent.is_empty());
    }
}
#[test]
fn image_digest_expansion_and_caller_bounds_are_independent() {
    let f = Fixture::new();
    let mut link = f.link();
    link.documents[1][8] ^= 1;
    assert!(matches!(
        observe_fixture(&f, &mut link),
        Err(Refusal::Document)
    ));
    let mut link = f.link();
    let mut presentation = [0; 4096];
    let mut images = [0; 8192];
    let mut decode = [0; 8192];
    let insufficient = Limits {
        maximum_expanded_offer_bytes: 8,
        ..limits()
    };
    assert!(matches!(
        ready(observe(
            &mut link,
            f.basis(),
            insufficient,
            ReceiveBuffers {
                presentation: &mut presentation,
                images: &mut images,
                decode: &mut decode,
            }
        )),
        Err(Refusal::Images(
            conduit_wire::CapabilityOfferImageRefusal::Bounds
        ))
    ));
    let mut link = f.link();
    let mut short = [0; 1];
    assert!(matches!(
        ready(observe(
            &mut link,
            f.basis(),
            limits(),
            ReceiveBuffers {
                presentation: &mut presentation,
                images: &mut images,
                decode: &mut short,
            }
        )),
        Err(Refusal::Bounds)
    ));
    assert_eq!(link.received_documents, 0);
    let mut invalid = limits();
    invalid.maximum_total_image_bytes = 24 * 1024 + 1;
    assert!(matches!(
        ready(observe(
            &mut link,
            f.basis(),
            invalid,
            ReceiveBuffers {
                presentation: &mut presentation,
                images: &mut images,
                decode: &mut decode,
            }
        )),
        Err(Refusal::Bounds)
    ));
    assert_eq!(link.received_documents, 0);
}
#[test]
fn wrong_challenge_proof_receipt_or_owner_cannot_be_delivered_as_acceptance() {
    let f = Fixture::new();
    let mut link = f.link();
    let observation = observe_fixture(&f, &mut link).unwrap();
    let mut frame = [0; 2048];
    let challenge = f.challenge();
    let mut wrong = challenge.clone();
    wrong.boot_id = f.prior.boot_id.clone();
    assert!(matches!(
        ready(prove(
            &mut link,
            f.basis(),
            &observation,
            &wrong,
            1001,
            &mut frame
        )),
        Err(Refusal::Challenge)
    ));
    assert!(link.sent.is_empty());
    assert!(matches!(
        ready(prove(
            &mut link,
            f.basis(),
            &observation,
            &challenge,
            2000,
            &mut frame
        )),
        Err(Refusal::Challenge)
    ));
    assert!(link.sent.is_empty());
    link.bad_proof = true;
    assert!(matches!(
        ready(prove(
            &mut link,
            f.basis(),
            &observation,
            &challenge,
            1001,
            &mut frame
        )),
        Err(Refusal::Signature)
    ));
    let mut oversized = f.receipt(observation.projection().clone());
    oversized.receipt_json = " ".repeat(16 * 1024 + 1);
    let count = link.sent.len();
    assert!(matches!(
        ready(deliver(
            &mut link,
            f.basis(),
            &observation,
            &oversized,
            &mut frame
        )),
        Err(Refusal::Bounds)
    ));
    assert_eq!(link.sent.len(), count);
    let mut signed = f.receipt(observation.projection().clone());
    signed.owner.boot_id = "wrong/owner-boot".into();
    let count = link.sent.len();
    assert!(matches!(
        ready(deliver(
            &mut link,
            f.basis(),
            &observation,
            &signed,
            &mut frame
        )),
        Err(Refusal::Receipt)
    ));
    assert_eq!(link.sent.len(), count);
    let mut wrong_projection = observation.projection().clone();
    wrong_projection.capability_images[0][0] ^= 1;
    let signed = f.receipt(wrong_projection);
    assert!(matches!(
        ready(deliver(
            &mut link,
            f.basis(),
            &observation,
            &signed,
            &mut frame
        )),
        Err(Refusal::Receipt)
    ));
    assert_eq!(link.sent.len(), count);
    let signed = f.receipt(observation.projection().clone());
    link.fail_send = true;
    assert!(matches!(
        ready(deliver(
            &mut link,
            f.basis(),
            &observation,
            &signed,
            &mut frame
        )),
        Err(Refusal::Transport)
    ));
    link.fail_send = false;
    assert_eq!(
        ready(deliver(
            &mut link,
            f.basis(),
            &observation,
            &signed,
            &mut frame
        ))
        .unwrap()
        .part_id,
        f.prior.part_id
    );
}
