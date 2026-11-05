use conduit_wire::{
    BorrowedBoundedDocumentAssembly, BoundedDocumentError as E,
    SegmentedBoundedDocument as Document, BOUNDED_DOCUMENT_HEADER_BYTES as HEADER,
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

#[test]
fn every_split_offset_and_frame_size_preserves_the_cbd1_bytes() {
    let bytes = b"a finite document spanning immutable fragments";
    for split in 0..=bytes.len() {
        let segments = [&bytes[..split], &[], &bytes[split..], &[]];
        let document = Document::new(&segments, bytes.len(), 4).unwrap();
        assert_eq!(document.total_bytes(), bytes.len());
        for offset in 0..bytes.len() {
            for size in [HEADER + 1, HEADER + 7, HEADER + bytes.len()] {
                let mut frame = [0xa5; 128];
                let (length, next) = document
                    .encode_chunk(&mut frame[..size], offset, &[7; 32])
                    .unwrap();
                let end = bytes.len().min(offset + size - HEADER);
                assert_eq!(next, end);
                assert_eq!(length, HEADER + end - offset);
                assert_eq!(&frame[..4], b"CBD1");
                assert_eq!(&frame[4..8], &(bytes.len() as u32).to_le_bytes());
                assert_eq!(&frame[8..12], &(offset as u32).to_le_bytes());
                assert_eq!(&frame[12..HEADER], &[7; 32]);
                assert_eq!(&frame[HEADER..length], &bytes[offset..end]);
                assert!(frame[length..].iter().all(|b| *b == 0xa5));
            }
        }
    }
}

#[test]
fn transmit_and_receive_borrowed_segments_without_a_full_document_copy_or_allocation() {
    static FLASH: [u8; 8193] = [0x5a; 8193];
    let header = [1, 2, 3];
    let segments = [&header[..], &FLASH[..], &[4, 5][..]];
    let mut recovered = [0; 8198];
    let ((), cost) = probe::observe(|| {
        let document = Document::new(&segments, 8198, 3).unwrap();
        let mut receiver = BorrowedBoundedDocumentAssembly::new(&mut recovered, 3);
        let mut frame = [0; 4096];
        let mut offset = 0;
        while offset < document.total_bytes() {
            let (length, next) = document.encode_chunk(&mut frame, offset, &[7; 32]).unwrap();
            assert_eq!(receiver.push(&frame[..length]).unwrap(), next == 8198);
            offset = next;
        }
        let (bytes, _) = receiver.completed().unwrap();
        assert_eq!(&bytes[..3], &header);
        assert_eq!(&bytes[3..8196], &FLASH);
        assert_eq!(&bytes[8196..], &[4, 5]);
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}

#[test]
fn invalid_extents_or_segment_budgets_refuse_without_touching_the_frame() {
    assert!(matches!(Document::new(&[], 10, 1), Err(E::Bound)));
    assert!(matches!(Document::new(&[&[], &[]], 10, 2), Err(E::Bound)));
    let segments = [&b"abc"[..], &b"def"[..]];
    assert!(matches!(Document::new(&segments, 5, 2), Err(E::Bound)));
    assert!(matches!(Document::new(&segments, 6, 1), Err(E::Bound)));
    assert!(matches!(Document::new(&segments, 6, 0), Err(E::Bound)));
    let document = Document::new(&segments, 6, 2).unwrap();
    for (size, offset) in [(0, 0), (HEADER, 0), (64, 6), (64, usize::MAX)] {
        let mut frame = [0xa5; 64];
        assert_eq!(
            document.encode_chunk(&mut frame[..size], offset, &[7; 32]),
            Err(E::Bound)
        );
        assert_eq!(frame, [0xa5; 64]);
    }
}
