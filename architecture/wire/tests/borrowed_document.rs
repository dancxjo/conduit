use conduit_wire::{
    encode_bounded_document_chunk as encode, BorrowedBoundedDocumentAssembly as Borrowed,
    BoundedDocumentAssembly as Owned, BoundedDocumentError as E,
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

#[test]
fn receive_larger_than_one_frame_into_exact_caller_workspace_without_allocation() {
    let document = [0x5a; 6145];
    let mut arena = [0xa5; 6177];
    let address = arena.as_ptr().wrapping_add(16);
    let ((), cost) = probe::observe(|| {
        let mut assembly = Borrowed::new(&mut arena[16..6161], 2);
        let mut frame = [0; 4096];
        assert!(assembly.completed().is_none());
        let (length, next) = encode(&mut frame, &document, 0, &[7; 32]).unwrap();
        assert!(!assembly.push(&frame[..length]).unwrap());
        assert!(assembly.completed().is_none());
        let (length, next) = encode(&mut frame, &document, next, &[7; 32]).unwrap();
        assert_eq!(next, document.len());
        assert!(assembly.push(&frame[..length]).unwrap());
        let (received, digest) = assembly.completed().unwrap();
        assert_eq!(received, document);
        assert_eq!(digest, &[7; 32]); // Integrity/authentication remain the caller's job.
        assert_eq!(received.as_ptr(), address);
        assert_eq!(assembly.push(&frame[..length]), Err(E::ChunkCount));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(&arena[..16], &[0xa5; 16]);
    assert_eq!(&arena[6161..], &[0xa5; 16]);
}

#[test]
fn both_storage_forms_refuse_the_same_frames_and_retain_partial_input() {
    let document = [0x5a; 20];
    let mut first = [0; 60];
    let (first_length, next) = encode(&mut first, &document, 0, &[7; 32]).unwrap();
    let mut last = [0; 60];
    let (last_length, _) = encode(&mut last, &document, next, &[7; 32]).unwrap();
    let mut cases = Vec::new();
    for (at, value, error) in [
        (0, b'X', E::Header),
        (4, 0, E::Bound),
        (4, 33, E::Bound),
        (4, 21, E::Identity),
        (8, 15, E::Offset),
        (12, 8, E::Identity),
    ] {
        let mut bad = last[..last_length].to_vec();
        bad[at] = value;
        cases.push((bad, error));
    }
    cases.push((last[..44].to_vec(), E::Header));
    cases.push((last[..last_length + 1].to_vec(), E::Bound));
    let mut unrepresentable_extent = last[..last_length].to_vec();
    unrepresentable_extent[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push((unrepresentable_extent, E::Bound));
    for (bad, error) in cases {
        let mut storage = [0xa5; 32];
        let mut borrowed = Borrowed::new(&mut storage, 4);
        let mut owned = Owned::new(32, 4);
        assert!(!borrowed.push(&first[..first_length]).unwrap());
        assert!(!owned.push(&first[..first_length]).unwrap());
        assert_eq!(borrowed.push(&bad), Err(error));
        assert_eq!(owned.push(&bad), Err(error));
        assert!(borrowed.completed().is_none());
        assert!(owned.completed().is_none());
        assert!(borrowed.push(&last[..last_length]).unwrap());
        assert!(owned.push(&last[..last_length]).unwrap());
        assert_eq!(borrowed.completed(), owned.completed());
        assert_eq!(borrowed.completed().unwrap().0, document);
        assert_eq!(borrowed.push(&last[..last_length]), Err(E::Offset));
    }
}

#[test]
fn undersized_or_exhausted_workspace_never_grows_or_accepts_partial_completion() {
    let mut frame = [0; 60];
    let (n, _) = encode(&mut frame, &[1; 20], 0, &[7; 32]).unwrap();
    let mut tiny = [0xa5; 19];
    {
        let mut assembly = Borrowed::new(&mut tiny, 2);
        assert_eq!(assembly.push(&frame[..n]), Err(E::Bound));
        assert!(assembly.completed().is_none());
    }
    assert_eq!(tiny, [0xa5; 19]);
    let mut storage = [0; 20];
    let mut assembly = Borrowed::new(&mut storage, 1);
    assert!(!assembly.push(&frame[..n]).unwrap());
    let (n, _) = encode(&mut frame, &[1; 20], 16, &[7; 32]).unwrap();
    assert_eq!(assembly.push(&frame[..n]), Err(E::ChunkCount));
    assert!(assembly.completed().is_none());
    let mut empty = [];
    assert_eq!(
        Borrowed::new(&mut empty, 1).push(&frame[..n]),
        Err(E::Bound)
    );
    assert_eq!(
        Borrowed::new(&mut storage, 0).push(&frame[..n]),
        Err(E::ChunkCount)
    );
}
