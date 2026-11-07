use conduit_body::{
    AdmissionReceiptAttestationRefusal as R, AdmissionReceiptStreamBounds as Bounds,
    AdmissionReceiptTranscript, BorrowedAdmissionReceiptAttestation, BorrowedFederationPeer,
    FederationPeer, MAX_ATTESTED_ADMISSION_RECEIPT_BYTES as MAX,
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

fn peer(key: &SigningKey) -> FederationPeer {
    FederationPeer {
        host_id: "fixture/owner".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: conduit_core::OfferGeneration(7),
        verifying_key: key.verifying_key().to_bytes(),
    }
}

// Independent reconstruction of the pre-streaming transcript, not the new API.
fn reference(bytes: &[u8], owner: &FederationPeer, member: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in [
        &b"conduit.body/admission-receipt-attestation@1"[..],
        owner.host_id.as_str().as_bytes(),
        owner.boot_id.as_str().as_bytes(),
        bytes,
    ] {
        hash.update((part.len() as u32).to_le_bytes());
        hash.update(part);
    }
    hash.update(owner.offer_generation.0.to_le_bytes());
    hash.update(owner.verifying_key);
    hash.update(member);
    hash.finalize().into()
}

fn metadata<'a>(
    owner: &'a FederationPeer,
    member: [u8; 32],
    signature: [u8; 64],
) -> BorrowedAdmissionReceiptAttestation<'a> {
    BorrowedAdmissionReceiptAttestation {
        schema: "conduit.body/admission-receipt-attestation@1",
        owner: BorrowedFederationPeer {
            host_id: owner.host_id.as_str(),
            boot_id: owner.boot_id.as_str(),
            offer_generation: owner.offer_generation.0,
            verifying_key: owner.verifying_key,
        },
        member_verifying_key: member,
        signature,
    }
}

#[test]
fn every_split_preserves_the_existing_signed_transcript_without_allocation() {
    let bytes = br#"{"fixture":"transport evidence only","sequence":17}"#;
    let key = SigningKey::from_bytes(&[7; 32]);
    let owner = peer(&key);
    let member = [9; 32];
    let expected = reference(bytes, &owner, &member);
    let meta = metadata(&owner, member, key.sign(&expected).to_bytes());
    for split in 1..bytes.len() {
        let ((), cost) = probe::observe(|| {
            let bounds = || Bounds {
                receipt_bytes: bytes.len(),
                maximum_chunks: 2,
            };
            let mut transcript = AdmissionReceiptTranscript::new(
                bounds(),
                owner.host_id.as_str(),
                owner.boot_id.as_str(),
                owner.offer_generation.0,
                &owner.verifying_key,
                &member,
            )
            .unwrap();
            transcript.push(&bytes[..split]).unwrap();
            transcript.push(&bytes[split..]).unwrap();
            assert_eq!(transcript.finish().unwrap(), expected);
            let mut verifier = meta.begin_verification(bounds(), &owner, &member).unwrap();
            verifier.push(&bytes[..split]).unwrap();
            verifier.push(&bytes[split..]).unwrap();
            verifier.finish().unwrap();
        });
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
    }
    meta.verify(bytes, &owner, &member).unwrap();
}

#[test]
fn a_full_profile_receipt_verifies_without_retaining_or_decoding_a_document() {
    static BYTES: [u8; MAX] = [0x5a; MAX]; // Authentication fixture, not a semantic receipt.
    let key = SigningKey::from_bytes(&[7; 32]);
    let owner = peer(&key);
    let member = [9; 32];
    let meta = metadata(
        &owner,
        member,
        key.sign(&reference(&BYTES, &owner, &member)).to_bytes(),
    );
    let ((), cost) = probe::observe(|| {
        let mut verifier = meta
            .begin_verification(
                Bounds {
                    receipt_bytes: MAX,
                    maximum_chunks: 16,
                },
                &owner,
                &member,
            )
            .unwrap();
        for chunk in BYTES.chunks(1024) {
            verifier.push(chunk).unwrap();
        }
        verifier.finish().unwrap();
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(MAX, 16 * 1024);
    for (bytes, chunks) in [(0, 1), (MAX + 1, 1), (MAX, 0), (MAX, MAX + 1)] {
        assert!(matches!(
            meta.begin_verification(
                Bounds {
                    receipt_bytes: bytes,
                    maximum_chunks: chunks
                },
                &owner,
                &member,
            ),
            Err(R::Bounds)
        ));
    }
}

#[test]
fn refused_chunks_preserve_the_accepted_prefix_and_incomplete_inputs_cannot_finish() {
    let bytes = b"{}\n";
    let key = SigningKey::from_bytes(&[7; 32]);
    let owner = peer(&key);
    let member = [9; 32];
    let meta = metadata(
        &owner,
        member,
        key.sign(&reference(bytes, &owner, &member)).to_bytes(),
    );
    let bounds = || Bounds {
        receipt_bytes: bytes.len(),
        maximum_chunks: 2,
    };
    let mut verifier = meta.begin_verification(bounds(), &owner, &member).unwrap();
    verifier.push(&bytes[..1]).unwrap();
    assert_eq!(verifier.push(b""), Err(R::Bounds));
    assert_eq!(verifier.push(bytes), Err(R::Bounds));
    verifier.push(&bytes[1..]).unwrap();
    assert_eq!(verifier.push(b"x"), Err(R::Bounds));
    verifier.finish().unwrap();
    let mut verifier = meta.begin_verification(bounds(), &owner, &member).unwrap();
    verifier.push(&bytes[..1]).unwrap();
    assert_eq!(verifier.finish(), Err(R::Bounds));
    let mut verifier = meta
        .begin_verification(
            Bounds {
                receipt_bytes: bytes.len(),
                maximum_chunks: 1,
            },
            &owner,
            &member,
        )
        .unwrap();
    verifier.push(&bytes[..1]).unwrap();
    assert_eq!(verifier.push(&bytes[1..]), Err(R::Bounds));
    assert_eq!(verifier.finish(), Err(R::Bounds));
}

#[test]
fn independent_pins_byte_order_contents_and_signature_cannot_be_substituted() {
    let bytes = b"123456";
    let key = SigningKey::from_bytes(&[7; 32]);
    let owner = peer(&key);
    let member = [9; 32];
    let mut meta = metadata(
        &owner,
        member,
        key.sign(&reference(bytes, &owner, &member)).to_bytes(),
    );
    let bounds = || Bounds {
        receipt_bytes: bytes.len(),
        maximum_chunks: 2,
    };
    for mutation in 0..4 {
        let mut other = owner.clone();
        match mutation {
            0 => other.host_id = "other/host".into(),
            1 => other.boot_id = "other/boot".into(),
            2 => other.offer_generation.0 += 1,
            _ => other.verifying_key = SigningKey::from_bytes(&[6; 32]).verifying_key().to_bytes(),
        }
        assert!(matches!(
            meta.begin_verification(bounds(), &other, &member),
            Err(R::Owner)
        ));
    }
    assert!(matches!(
        meta.begin_verification(bounds(), &owner, &[8; 32]),
        Err(R::MemberKey)
    ));
    for content in [b"456123", b"123457"] {
        let mut verifier = meta.begin_verification(bounds(), &owner, &member).unwrap();
        verifier.push(content).unwrap();
        assert_eq!(verifier.finish(), Err(R::Signature));
    }
    meta.signature[0] ^= 1;
    let mut verifier = meta.begin_verification(bounds(), &owner, &member).unwrap();
    verifier.push(bytes).unwrap();
    assert_eq!(verifier.finish(), Err(R::Signature));
    meta.schema = "other/protocol";
    assert!(matches!(
        meta.begin_verification(bounds(), &owner, &member),
        Err(R::Owner)
    ));
}
