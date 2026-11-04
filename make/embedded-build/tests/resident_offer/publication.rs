//! CPU wire proof using metadata borrowed from the generated initialized owners.
use conduit_wire::{BorrowedBoundedDocumentAssembly, SegmentedBoundedDocument};
use sha2::{Digest, Sha256};

pub fn check(offers: [Option<&[u8]>; 2]) {
    let comma: &[u8] = if offers.iter().all(Option::is_some) {
        b","
    } else {
        b""
    };
    // This is the ordinary JSON array of CapabilityOffers, not an advertisement,
    // membership proof, authority, resource pool or offer-generation publication.
    let segments = [
        &b"["[..],
        offers[0].unwrap_or(b""),
        comma,
        offers[1].unwrap_or(b""),
        &b"]"[..],
    ];
    let maximum = segments.iter().map(|part| part.len()).sum();
    // Hosted receive fixture only; the sender does not own a flattened document.
    let mut storage = vec![0; maximum];
    let ((digest, chunks), cost) = crate::probe::observe(|| {
        let document = SegmentedBoundedDocument::new(&segments, maximum, 5).unwrap();
        let mut digest = Sha256::new();
        for segment in segments {
            digest.update(segment);
        }
        let digest: [u8; 32] = digest.finalize().into();
        let mut receiver = BorrowedBoundedDocumentAssembly::new(&mut storage, 1024);
        let mut frame = [0; 128];
        let mut offset = 0;
        let mut chunks = 0;
        while offset < document.total_bytes() {
            let (length, next) = document.encode_chunk(&mut frame, offset, &digest).unwrap();
            assert_eq!(receiver.push(&frame[..length]).unwrap(), next == maximum);
            offset = next;
            chunks += 1;
        }
        assert!(chunks > 1);
        let (received, expected) = receiver.completed().unwrap();
        assert_eq!(Sha256::digest(received).as_slice(), expected);
        (digest, chunks)
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(Sha256::digest(&storage).as_slice(), digest);
    let received: Vec<conduit_core::CapabilityOffer> = serde_json::from_slice(&storage).unwrap();
    let expected: Vec<conduit_core::CapabilityOffer> = offers
        .into_iter()
        .flatten()
        .map(|bytes| serde_json::from_slice(bytes).unwrap())
        .collect();
    assert_eq!(received, expected);
    println!(
        "resident offer document: {} offers, {maximum} bytes, {chunks} CBD1 chunks, 128-byte frame, zero scoped allocations",
        received.len()
    );
}
