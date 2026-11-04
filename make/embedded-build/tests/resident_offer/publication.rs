//! CPU wire proof using metadata borrowed from the generated initialized owners.
use conduit_wire::{BorrowedBoundedDocumentAssembly, SegmentedBoundedDocument};
use sha2::{Digest, Sha256};

pub fn decode(image: &[u8]) -> conduit_core::CapabilityOffer {
    let document = conduit_wire::DeflatedDocument::parse_image(image).unwrap();
    // Hosted fixture workspace; these limits are independent of the image.
    let mut workspace = vec![0; 256 * 1024];
    let raw = document
        .decode_into(
            &mut workspace,
            conduit_wire::DeflatedDocumentLimits {
                maximum_expanded_bytes: 256 * 1024,
                maximum_compressed_bytes: 32 * 1024,
            },
        )
        .unwrap();
    serde_json::from_slice(raw).unwrap()
}

pub fn check(offers: [Option<&[u8]>; 2]) {
    let mut total_bytes = 0;
    let mut total_chunks = 0;
    for image in offers.into_iter().flatten() {
        // Lend the image header and compressed bytes, not a synthetic envelope.
        let segments = [&image[..8], &image[8..]];
        let mut storage = vec![0; image.len()]; // Hosted receive fixture only.
        let (chunks, cost) = crate::probe::observe(|| {
            let document = SegmentedBoundedDocument::new(&segments, image.len(), 2).unwrap();
            let digest: [u8; 32] = Sha256::digest(image).into();
            let mut receiver = BorrowedBoundedDocumentAssembly::new(&mut storage, 1024);
            let mut frame = [0; 128];
            let mut offset = 0;
            let mut chunks = 0;
            while offset < document.total_bytes() {
                let (length, next) = document.encode_chunk(&mut frame, offset, &digest).unwrap();
                assert_eq!(
                    receiver.push(&frame[..length]).unwrap(),
                    next == image.len()
                );
                offset = next;
                chunks += 1;
            }
            assert!(chunks > 1);
            let (received, expected) = receiver.completed().unwrap();
            assert_eq!(received, image);
            assert_eq!(Sha256::digest(received).as_slice(), expected);
            chunks
        });
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert_eq!(decode(&storage), decode(image));
        total_bytes += image.len();
        total_chunks += chunks;
    }
    println!(
        "resident offer images: {} offers, {total_bytes} bytes, {total_chunks} CBD1 chunks, 128-byte frame, zero scoped allocations",
        offers.iter().flatten().count()
    );
}
