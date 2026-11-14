use crate::{
    member_return::{Refusal, RetainedReturn},
    tests::ready,
    transport::{DocumentTransport, MessageTransport},
};
use conduit_body::*;
use conduit_core::{BootId, HostAdvertisement};
use ed25519_dalek::{Signature, SigningKey};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, vec, vec::Vec};
const IMAGE: &[u8] = b"immutable-offer-image";
struct Messages {
    incoming: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
}
impl MessageTransport for Messages {
    type Error = ();
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let bytes = self.incoming.pop_front().ok_or(())?;
        let out = output.get_mut(..bytes.len()).ok_or(())?;
        out.copy_from_slice(&bytes);
        Ok(out)
    }
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.sent.push(bytes.to_vec());
        Ok(())
    }
}
impl DocumentTransport for Messages {
    async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        let mut frame = [0; 4096];
        loop {
            let bytes = self.receive(&mut frame).await?;
            if document.push(bytes).map_err(|_| ())? {
                return Ok(());
            }
        }
    }
}
fn fixture() -> (
    MemberBootstrap<'static>,
    HostAdvertisement,
    SigningKey,
    SigningKey,
) {
    let (mut bootstrap, actual, member, owner) = crate::membership_tests::fixture();
    let mut retained = crate::membership_tests::receipt(&bootstrap, &actual).credential;
    retained.boot_id = BootId::from("previous/boot");
    bootstrap.admission = MemberBootstrapMaterial::Retained {
        credential: retained,
        secret: [3; 32],
    };
    (bootstrap, actual, member, owner)
}
fn challenge<'a>(actual: &'a HostAdvertisement) -> BorrowedPartReturnChallenge<'a> {
    BorrowedPartReturnChallenge {
        admission_id: "admission/return",
        body_id: "body/1",
        part_id: "part/1",
        host_id: actual.host_id.as_str(),
        boot_id: actual.boot_id.as_str(),
        offer_generation: actual.offer_generation.0,
        nonce: [8; 32],
        issued_at_millis: 100,
        expires_at_millis: 200,
    }
}
fn messages(
    bootstrap: &MemberBootstrap<'_>,
    actual: &HostAdvertisement,
    member: &SigningKey,
    owner: &SigningKey,
) -> Messages {
    let retained = bootstrap.admission.credential().unwrap();
    let mut credential = retained.clone();
    credential.boot_id = actual.boot_id.clone();
    let projection = HostAdvertisementImageProjection {
        context: actual.clone(),
        capability_images: vec![Sha256::digest(IMAGE).into()],
        authority_grants: vec![],
    };
    use ed25519_dalek::Signer;
    let signed = attest_member_return_image_acknowledgement(
        &MemberReturnImageAcknowledgement::new(credential, projection),
        bootstrap.owner.clone(),
        member.verifying_key().to_bytes(),
        |t| owner.sign(t).to_bytes(),
    )
    .unwrap();
    let document = signed.receipt_json.as_bytes();
    let digest = Sha256::digest(document).into();
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < document.len() {
        let mut frame = [0; 4096];
        let (length, next) =
            conduit_wire::encode_bounded_document_chunk(&mut frame, document, offset, &digest)
                .unwrap();
        chunks.push(frame[..length].to_vec());
        offset = next;
    }
    Messages {
        incoming: [
            serde_json::to_vec(&challenge(actual)).unwrap(),
            serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap(),
        ]
        .into_iter()
        .chain(chunks)
        .collect(),
        sent: vec![],
    }
}
#[test]
fn retained_return_preserves_part_and_signs_exact_expiry_bearing_challenge() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let mut link = messages(&bootstrap, &actual, &member, &owner);
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    let accepted = ready(exchange.exchange(
        &mut link,
        [IMAGE].into_iter(),
        || Ok(vec![]),
        &mut [0; 4096],
        &mut [0; 4096],
    ))
    .unwrap();
    assert_eq!(accepted.credential.body_id, retained.body_id);
    assert_eq!(accepted.credential.part_id, retained.part_id);
    assert_eq!(accepted.credential.boot_id, actual.boot_id);
    assert_eq!(retained.boot_id.as_str(), "previous/boot");
    let mut presentation_bytes = conduit_wire::BoundedDocumentAssembly::new(16 * 1024, 32);
    let mut frame = 0;
    loop {
        let complete = presentation_bytes.push(&link.sent[frame]).unwrap();
        frame += 1;
        if complete {
            break;
        }
    }
    assert_eq!(link.sent.len(), frame + 2);
    let presentation: MemberReturnImagePresentation =
        serde_json::from_slice(presentation_bytes.completed().unwrap().0).unwrap();
    presentation
        .verify(&member.verifying_key().to_bytes())
        .unwrap();
    let mut output = [0; 128];
    let mut document = conduit_wire::BorrowedBoundedDocumentAssembly::new(&mut output, 1);
    assert!(document.push(&link.sent[frame]).unwrap());
    assert_eq!(document.completed().unwrap().0, IMAGE);
    let proof: serde_json::Value = serde_json::from_slice(&link.sent[frame + 1]).unwrap();
    let signature: Vec<u8> = serde_json::from_value(proof["signature"].clone()).unwrap();
    member
        .verifying_key()
        .verify_strict(
            &challenge(&actual).signing_transcript(),
            &Signature::from_slice(&signature).unwrap(),
        )
        .unwrap();
    let mut changed = challenge(&actual);
    changed.expires_at_millis += 1;
    assert!(member
        .verifying_key()
        .verify_strict(
            &changed.signing_transcript(),
            &Signature::from_slice(&signature).unwrap()
        )
        .is_err());
}
#[test]
fn stale_challenge_or_invalid_expiry_refuses_before_return_proof() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    for stale_boot in [true, false] {
        let mut link = messages(&bootstrap, &actual, &member, &owner);
        let mut request = challenge(&actual);
        if stale_boot {
            request.boot_id = "old/boot";
        } else {
            request.expires_at_millis = request.issued_at_millis;
        }
        link.incoming[0] = serde_json::to_vec(&request).unwrap();
        assert!(matches!(
            ready(exchange.exchange(
                &mut link,
                [IMAGE].into_iter(),
                || Ok(vec![]),
                &mut [0; 4096],
                &mut [0; 4096]
            )),
            Err(Refusal::Challenge)
        ));
        assert_eq!(link.sent.len(), 2);
    }
}
#[test]
fn lost_owner_ack_or_wrong_owner_key_cannot_install_returned_membership() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    let mut interrupted = messages(&bootstrap, &actual, &member, &owner);
    interrupted.incoming.truncate(1);
    assert!(matches!(
        ready(exchange.exchange(
            &mut interrupted,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Transport)
    ));
    let wrong = SigningKey::from_bytes(&[6; 32]);
    let mut forged = messages(&bootstrap, &actual, &member, &wrong);
    assert!(matches!(
        ready(exchange.exchange(
            &mut forged,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Attestation)
    ));
    assert_eq!(retained.boot_id.as_str(), "previous/boot");
}
#[test]
fn changed_image_or_retained_body_refuses_before_carrier_effects() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let mut link = messages(&bootstrap, &actual, &member, &owner);
    let mut exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "other/image",
        key: &member,
        retained,
    };
    assert!(matches!(
        ready(exchange.exchange(
            &mut link,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Scope)
    ));
    let mut changed = retained.clone();
    changed.body_id = serde_json::from_str(r#""other/body""#).unwrap();
    exchange.image_id = "image/1";
    exchange.retained = &changed;
    assert!(matches!(
        ready(exchange.exchange(
            &mut link,
            [IMAGE].into_iter(),
            || Ok(vec![]),
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Scope)
    ));
    assert!(link.sent.is_empty());
}

#[test]
fn excess_images_refuse_before_native_grant_copy_or_transport() {
    let (bootstrap, actual, member, owner) = fixture();
    let retained = bootstrap.admission.credential().unwrap();
    let exchange = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained,
    };
    let mut link = messages(&bootstrap, &actual, &member, &owner);
    let copied = core::cell::Cell::new(false);
    assert!(matches!(
        ready(exchange.exchange(
            &mut link,
            [IMAGE; conduit_body::MAX_MEMBER_RETURN_IMAGES + 1].into_iter(),
            || {
                copied.set(true);
                Ok(vec![])
            },
            &mut [0; 4096],
            &mut [0; 4096]
        )),
        Err(Refusal::Bounds)
    ));
    assert!(!copied.get());
    assert!(link.sent.is_empty());
}

// A borrowed test carrier never allocates or releases prebuilt message owners
// inside the measured transaction. Sent frames are counted, not copied.
struct BorrowedMessages<'a> {
    incoming: &'a [Vec<u8>],
    next: usize,
    sent: usize,
}
impl MessageTransport for BorrowedMessages<'_> {
    type Error = ();
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let bytes = self.incoming.get(self.next).ok_or(())?;
        let out = output.get_mut(..bytes.len()).ok_or(())?;
        out.copy_from_slice(bytes);
        self.next += 1;
        Ok(out)
    }
    async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        self.sent += 1;
        Ok(())
    }
}
impl DocumentTransport for BorrowedMessages<'_> {
    async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        loop {
            let bytes = self.incoming.get(self.next).ok_or(())?;
            self.next += 1;
            if document.push(bytes).map_err(|_| ())? {
                return Ok(());
            }
        }
    }
}
pub(crate) mod return_allocation_probe {
    use std::thread_local;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../architecture/plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static RETURN_ALLOCATOR: return_allocation_probe::Allocator = return_allocation_probe::Allocator;

#[test]
fn signed_return_transaction_measures_all_dynamic_owners() {
    let (bootstrap, actual, member, owner) = fixture();
    let incoming: Vec<_> = messages(&bootstrap, &actual, &member, &owner)
        .incoming
        .into_iter()
        .collect();
    let mut link = BorrowedMessages {
        incoming: &incoming,
        next: 0,
        sent: 0,
    };
    let transaction = RetainedReturn {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        retained: bootstrap.admission.credential().unwrap(),
    };
    let ((accepted, future_bytes), cost) = return_allocation_probe::observe(|| {
        let mut input = vec![0; 9 * 1024];
        let mut output = vec![0; 4096];
        let exchange = transaction.exchange(
            &mut link,
            core::iter::once(IMAGE),
            || Ok(Vec::new()),
            &mut input,
            &mut output,
        );
        let future_bytes = core::mem::size_of_val(&exchange);
        (ready(exchange).unwrap(), future_bytes)
    });
    assert_eq!(accepted.credential.boot_id, actual.boot_id);
    assert!(link.sent > 0);
    std::println!("signed-transaction fixture_images=1 grants=0 input=9216 output=4096 dynamic_peak={} allocations={} future_bytes={}",cost.peak_bytes,cost.allocations,future_bytes);
}

#[test]
#[ignore = "requires CONDUIT_PUBLIC_RETURN_RECEIPT public wire calibration input"]
fn public_shape_signed_return_measures_whole_transaction() {
    use ed25519_dalek::Signer;
    let output_bytes = std::env::var("CONDUIT_PUBLIC_RETURN_OUTPUT_BYTES")
        .map(|text| text.parse::<usize>().expect("output byte count"))
        .unwrap_or(4096);
    let path = std::env::var_os("CONDUIT_PUBLIC_RETURN_RECEIPT").expect("public receipt path");
    let outer = std::fs::read(path).unwrap();
    let capture: SignedPortableAdmissionReceipt = serde_json::from_slice(&outer).unwrap();
    let mut ack: MemberReturnImageAcknowledgement =
        serde_json::from_str(&capture.receipt_json).unwrap();
    // Preserve the public current context, grant descriptions and credential
    // lengths. Only immutable image bytes and signing keys are CPU test fixtures;
    // this does not claim to replay the private member's live authentication.
    let images: Vec<Vec<u8>> = (0..ack.projection.capability_images.len())
        .map(|i| vec![i as u8])
        .collect();
    ack.projection.capability_images = images
        .iter()
        .map(|bytes| Sha256::digest(bytes).into())
        .collect();
    let (mut bootstrap, _, member, owner) = fixture();
    let actual = &ack.projection.context;
    bootstrap.body_id = ack.credential.body_id.clone();
    bootstrap.host_id = actual.host_id.as_str().into();
    bootstrap.configuration_id = actual.profile.as_str().into();
    bootstrap.admission = MemberBootstrapMaterial::Retained {
        credential: ack.credential.clone(),
        secret: [3; 32],
    };
    let challenge = BorrowedPartReturnChallenge {
        admission_id: "admission/return",
        body_id: ack.credential.body_id.as_str(),
        part_id: ack.credential.part_id.as_str(),
        host_id: actual.host_id.as_str(),
        boot_id: actual.boot_id.as_str(),
        offer_generation: actual.offer_generation.0,
        nonce: [8; 32],
        issued_at_millis: 100,
        expires_at_millis: 200,
    };
    let signed = attest_member_return_image_acknowledgement(
        &ack,
        bootstrap.owner.clone(),
        member.verifying_key().to_bytes(),
        |t| owner.sign(t).to_bytes(),
    )
    .unwrap();
    let document = signed.receipt_json.as_bytes();
    let digest = Sha256::digest(document).into();
    let mut incoming = vec![
        serde_json::to_vec(&challenge).unwrap(),
        serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap(),
    ];
    let mut offset = 0;
    while offset < document.len() {
        let mut frame = vec![0; output_bytes];
        let (n, next) =
            conduit_wire::encode_bounded_document_chunk(&mut frame, document, offset, &digest)
                .unwrap();
        incoming.push(frame[..n].to_vec());
        offset = next;
    }
    let mut link = BorrowedMessages {
        incoming: &incoming,
        next: 0,
        sent: 0,
    };
    let transaction = RetainedReturn {
        bootstrap: &bootstrap,
        actual,
        image_id: "image/1",
        key: &member,
        retained: bootstrap.admission.credential().unwrap(),
    };
    let ((accepted, future_bytes), cost) = return_allocation_probe::observe(|| {
        let mut input = vec![0; 9 * 1024];
        let mut output = vec![0; output_bytes];
        let exchange = transaction.exchange(
            &mut link,
            images.iter().map(Vec::as_slice),
            || Ok(ack.projection.authority_grants.clone()),
            &mut input,
            &mut output,
        );
        let future_bytes = core::mem::size_of_val(&exchange);
        (ready(exchange).unwrap(), future_bytes)
    });
    assert_eq!(accepted.credential, ack.credential);
    assert_eq!(link.next, incoming.len());
    std::println!("signed-transaction public_shape_outer={} fixture_ack={} images={} grants={} input=9216 output={} dynamic_peak={} allocations={} future_bytes={}", outer.len(), document.len(), images.len(), ack.projection.authority_grants.len(), output_bytes, cost.peak_bytes, cost.allocations, future_bytes);
    let references: Vec<_> = ack.projection.authority_grants.iter().collect();
    let mut borrowed_link = BorrowedMessages {
        incoming: &incoming,
        next: 0,
        sent: 0,
    };
    let ((borrowed, borrowed_future), borrowed_cost) = return_allocation_probe::observe(|| {
        let mut input = vec![0; 9 * 1024];
        let mut output = vec![0; output_bytes];
        let exchange = transaction.exchange_with_grants(
            &mut borrowed_link,
            images.iter().map(Vec::as_slice),
            &references,
            &mut input,
            &mut output,
        );
        let extent = core::mem::size_of_val(&exchange);
        (ready(exchange).unwrap(), extent)
    });
    assert_eq!(borrowed.credential, accepted.credential);
    assert_eq!(borrowed.receipt_digest, accepted.receipt_digest);
    assert_eq!(borrowed_link.next, link.next);
    assert_eq!(borrowed_link.sent, link.sent);
    assert!(borrowed_cost.peak_bytes < cost.peak_bytes);
    std::println!("borrowed-grants reference_backing={} dynamic_peak={} allocations={} future_bytes={} saved_peak={}", references.capacity() * core::mem::size_of::<&conduit_core::AuthorityGrant>(), borrowed_cost.peak_bytes, borrowed_cost.allocations, borrowed_future, cost.peak_bytes - borrowed_cost.peak_bytes);
}

#[path = "member_control_tests.rs"]
mod member_control_tests;

#[path = "member_return_bounded_tests.rs"]
mod bounded_tests;
