//! Actual finite allocator backing; no fallback heap is provided to CArena.
use conduit_zenoh_pico_embassy::{ArenaError, CArena};
use std::mem::MaybeUninit;

fn backing(bytes: usize) -> &'static mut [MaybeUninit<u8>] {
    Box::leak(vec![MaybeUninit::uninit(); bytes].into_boxed_slice())
}
fn binary(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i.wrapping_mul(131) % 256) as u8).collect()
}
#[test]
fn alignment_is_preserved_even_with_deliberately_unaligned_backing() {
    fn check<const ALIGN: usize>() {
        let storage = backing(4097);
        let offset = usize::from((storage.as_ptr() as usize).is_multiple_of(ALIGN));
        let storage = &mut storage[offset..];
        assert_ne!((storage.as_ptr() as usize) % ALIGN, 0);
        let begin = storage.as_mut_ptr() as usize;
        let end = begin + storage.len();
        let mut arena = CArena::<ALIGN>::new(storage).unwrap();
        let mut pointers = Vec::new();
        for size in [1, 3, 17, 65, 127] {
            let ptr = arena.allocate(size);
            assert!(!ptr.is_null());
            assert_eq!((ptr as usize) % ALIGN, 0);
            assert!((ptr as usize) >= begin && (ptr as usize) + size <= end);
            pointers.push(ptr);
        }
        let stats = arena.stats();
        assert_eq!(stats.requested, 213);
        assert_eq!(stats.allocations, 5);
        assert!(stats.used >= stats.requested + pointers.len() * ALIGN);
        assert!(stats.used <= stats.capacity);
        for ptr in pointers {
            // SAFETY: each pointer is a distinct live allocation from this arena.
            unsafe { arena.free(ptr) };
        }
        assert_eq!(arena.stats().used, 0);
        assert_eq!(arena.stats().requested, 0);
        assert_eq!(arena.stats().allocations, 0);
    }
    check::<16>();
    check::<32>();
}
#[test]
fn realloc_growth_shrink_null_and_zero_preserve_binary_prefix_and_accounting() {
    let mut arena = CArena::<16>::new(backing(4096)).unwrap();
    assert!(arena.allocate(0).is_null());
    // SAFETY: null is explicitly supported by the C-compatible API.
    unsafe { arena.free(std::ptr::null_mut()) };
    let expected = binary(257);
    // SAFETY: null requests a new allocation.
    let first = unsafe { arena.reallocate(std::ptr::null_mut(), expected.len()) };
    assert!(!first.is_null());
    // SAFETY: first owns 257 writable bytes; expected is a separate allocation.
    unsafe { std::ptr::copy_nonoverlapping(expected.as_ptr(), first, expected.len()) };
    // SAFETY: first is live and is not used after successful reallocation.
    let grown = unsafe { arena.reallocate(first, 700) };
    assert!(!grown.is_null());
    assert_eq!((grown as usize) % 16, 0);
    // SAFETY: successful growth retains at least the original 257 bytes.
    assert_eq!(
        unsafe { std::slice::from_raw_parts(grown, expected.len()) },
        expected
    );
    assert_eq!(arena.stats().requested, 700);
    assert_eq!(arena.stats().allocations, 1);
    // SAFETY: grown is this arena's sole live pointer.
    let shrunk = unsafe { arena.reallocate(grown, 19) };
    assert!(!shrunk.is_null());
    assert_eq!((shrunk as usize) % 16, 0);
    // SAFETY: successful shrink owns 19 bytes.
    assert_eq!(
        unsafe { std::slice::from_raw_parts(shrunk, 19) },
        &expected[..19]
    );
    assert_eq!(arena.stats().requested, 19);
    // SAFETY: size zero retires shrunk exactly once.
    assert!(unsafe { arena.reallocate(shrunk, 0) }.is_null());
    let stats = arena.stats();
    assert_eq!(stats.used, 0);
    assert_eq!(stats.requested, 0);
    assert_eq!(stats.allocations, 0);
    assert!(stats.peak_requested >= 700);
    assert!(stats.peak_used >= stats.peak_requested);
    assert!(stats.peak_allocations >= 1);
}
#[test]
fn overflow_and_exhausted_realloc_preserve_the_original_live_allocation() {
    let mut arena = CArena::<32>::new(backing(2048)).unwrap();
    let expected = binary(80);
    let original = arena.allocate(expected.len());
    assert!(!original.is_null());
    // SAFETY: original owns 80 bytes and expected does not alias the arena.
    unsafe { std::ptr::copy_nonoverlapping(expected.as_ptr(), original, expected.len()) };
    let mut pressure = Vec::new();
    loop {
        let ptr = arena.allocate(80);
        if ptr.is_null() {
            break;
        }
        pressure.push(ptr);
        assert!(pressure.len() < 64, "finite backing unexpectedly grew");
    }
    let before = arena.stats();
    assert!(arena.allocate(usize::MAX).is_null());
    // SAFETY: failure leaves original live, so both failed requests use its same pointer.
    assert!(unsafe { arena.reallocate(original, usize::MAX) }.is_null());
    assert!(unsafe { arena.reallocate(original, 1024) }.is_null());
    // SAFETY: failed reallocations leave all 80 original bytes owned and intact.
    assert_eq!(
        unsafe { std::slice::from_raw_parts(original, expected.len()) },
        expected
    );
    let after = arena.stats();
    assert_eq!(after.used, before.used);
    assert_eq!(after.requested, before.requested);
    assert_eq!(after.allocations, before.allocations);
    for ptr in pressure {
        // SAFETY: pressure contains unique still-live pointers, each freed once.
        unsafe { arena.free(ptr) };
    }
    // SAFETY: original remains live after both failed reallocations.
    unsafe { arena.free(original) };
    assert_eq!(arena.stats().used, 0);
    assert_eq!(arena.stats().requested, 0);
    assert_eq!(arena.stats().allocations, 0);
}
#[test]
fn finite_exhaustion_reuse_and_coalescing_never_retain_live_allocator_bytes() {
    let mut arena = CArena::<16>::new(backing(4096)).unwrap();
    let mut allocations = Vec::new();
    loop {
        let ptr = arena.allocate(64);
        if ptr.is_null() {
            break;
        }
        allocations.push(Some(ptr));
        assert!(
            allocations.len() < 100,
            "arena must refuse instead of falling back"
        );
    }
    assert!(allocations.len() > 4);
    // Fragment first, then free the interleaved remainder to exercise coalescing.
    for parity in [0, 1] {
        for i in (parity..allocations.len()).step_by(2) {
            // SAFETY: take guarantees a live allocation is retired once.
            unsafe { arena.free(allocations[i].take().unwrap()) };
        }
    }
    let stats = arena.stats();
    assert_eq!(stats.used, 0);
    assert_eq!(stats.requested, 0);
    assert_eq!(stats.allocations, 0);
    let large = arena.allocate(stats.capacity / 2);
    assert!(!large.is_null(), "freed adjacent allocations must coalesce");
    // SAFETY: large is this arena's sole live allocation.
    unsafe { arena.free(large) };
    assert_eq!(arena.stats().used, 0);
    assert!(arena.stats().peak_allocations > 4);
}
#[test]
fn invalid_alignment_and_insufficient_backing_refuse() {
    assert!(matches!(
        CArena::<0>::new(backing(256)),
        Err(ArenaError::Alignment)
    ));
    assert!(matches!(
        CArena::<3>::new(backing(256)),
        Err(ArenaError::Alignment)
    ));
    assert!(matches!(
        CArena::<16>::new(backing(0)),
        Err(ArenaError::TooSmall)
    ));
    assert!(matches!(
        CArena::<32>::new(backing(8)),
        Err(ArenaError::TooSmall)
    ));
}
