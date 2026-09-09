extern crate std;

use super::*;
use alloc::{vec, vec::Vec};

struct Fixture {
    arena: BootArena,
    // Owns the bytes until every test allocation is released.
    _storage: Vec<u8>,
}

impl Fixture {
    fn new(bytes: usize) -> Self {
        let mut storage = vec![0; bytes + 64];
        let start = (storage.as_mut_ptr() as usize + 31) & !31;
        let arena = BootArena::new();
        unsafe { arena.initialize(start, bytes).unwrap() };
        Self {
            arena,
            _storage: storage,
        }
    }
}

#[test]
fn released_adjacent_preparations_are_reusable_without_touching_live_objects() {
    let fixture = Fixture::new(4096);
    let arena = &fixture.arena;
    let layout = Layout::from_size_align(1024, 32).unwrap();
    let allocations: Vec<_> = (0..4).map(|_| unsafe { arena.alloc(layout) }).collect();
    assert!(allocations.iter().all(|pointer| !pointer.is_null()));
    unsafe {
        core::ptr::write_bytes(allocations[0], 0x51, 1024);
        core::ptr::write_bytes(allocations[3], 0xa2, 1024);
        assert!(arena.alloc(layout).is_null());
        arena.dealloc(allocations[2], layout);
        arena.dealloc(allocations[1], layout);
        let joined = Layout::from_size_align(2048, 32).unwrap();
        let middle = arena.alloc(joined);
        assert_eq!(middle, allocations[1]);
        core::ptr::write_bytes(middle, 0x7c, 2048);
        assert!(
            core::slice::from_raw_parts(allocations[0], 1024)
                .iter()
                .all(|byte| *byte == 0x51)
        );
        assert!(
            core::slice::from_raw_parts(allocations[3], 1024)
                .iter()
                .all(|byte| *byte == 0xa2)
        );
        arena.dealloc(middle, joined);
        arena.dealloc(allocations[0], layout);
        arena.dealloc(allocations[3], layout);
    }
    assert_eq!(arena.live_bytes(), 0);
    assert_eq!(arena.used(), 4096);
}

#[test]
fn thousands_of_mixed_alignment_refreshes_keep_a_bounded_live_population() {
    let fixture = Fixture::new(64 * 1024);
    let arena = &fixture.arena;
    let mut live: Vec<(*mut u8, Layout, u8)> = Vec::new();
    let mut random = 0x12ab_89ef_u32;
    for step in 0..12_000 {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        if !live.is_empty() && (live.len() >= 40 || random.is_multiple_of(3)) {
            let index = random as usize % live.len();
            let (pointer, layout, byte) = live.swap_remove(index);
            unsafe {
                assert!(
                    core::slice::from_raw_parts(pointer, layout.size())
                        .iter()
                        .all(|value| *value == byte)
                );
                arena.dealloc(pointer, layout);
            }
        } else {
            let size = (random as usize % 1024) + 1;
            let align = 1 << ((random >> 12) % 13);
            let layout = Layout::from_size_align(size, align).unwrap();
            let pointer = unsafe { arena.alloc(layout) };
            if pointer.is_null() {
                continue;
            }
            assert_eq!(pointer as usize % align, 0);
            for &(other, other_layout, _) in &live {
                assert!(
                    pointer as usize + size <= other as usize
                        || other as usize + other_layout.size() <= pointer as usize
                );
            }
            let byte = step as u8;
            unsafe { core::ptr::write_bytes(pointer, byte, size) };
            live.push((pointer, layout, byte));
        }
        assert!(arena.live_bytes() <= arena.capacity());
    }
    for (pointer, layout, byte) in live {
        unsafe {
            assert!(
                core::slice::from_raw_parts(pointer, layout.size())
                    .iter()
                    .all(|value| *value == byte)
            );
            arena.dealloc(pointer, layout);
        }
    }
    assert_eq!(arena.live_bytes(), 0);
    let entire = Layout::from_size_align(arena.capacity(), 32).unwrap();
    let pointer = unsafe { arena.alloc(entire) };
    assert!(!pointer.is_null(), "all fragments must coalesce");
    unsafe { arena.dealloc(pointer, entire) };
}

#[test]
fn seal_and_failed_reinitialization_preserve_the_original_admission() {
    let fixture = Fixture::new(4096);
    let arena = &fixture.arena;
    let layout = Layout::from_size_align(17, 16).unwrap();
    let pointer = unsafe { arena.alloc(layout) };
    assert!(!pointer.is_null());
    assert_eq!(
        unsafe { arena.initialize(1, 64) },
        Err(ArenaError::InvalidRange)
    );
    assert_eq!(arena.capacity(), 4096);
    let before = arena.seal();
    unsafe {
        assert!(arena.alloc(layout).is_null());
        arena.dealloc(pointer, layout);
        assert!(
            arena.alloc(layout).is_null(),
            "freeing memory must not unseal"
        );
    }
    assert_eq!(arena.used(), before);
    assert_eq!(arena.live_bytes(), 0);
}

#[test]
fn invalid_ranges_and_unrepresentable_requests_refuse_without_mutation() {
    let arena = BootArena::new();
    for (start, bytes) in [
        (0, 64),
        (32, 0),
        (usize::MAX - 10, 64),
        (32, MAXIMUM_ARENA_BYTES + 1),
    ] {
        assert_eq!(
            unsafe { arena.initialize(start, bytes) },
            Err(ArenaError::InvalidRange)
        );
        assert_eq!(arena.capacity(), 0);
    }
    assert!(unsafe { arena.alloc(Layout::from_size_align(1, 1).unwrap()) }.is_null());
    let fixture = Fixture::new(4096);
    let huge = Layout::from_size_align(isize::MAX as usize, 1).unwrap();
    assert!(unsafe { fixture.arena.alloc(huge) }.is_null());
    assert_eq!(fixture.arena.live_bytes(), 0);
}

#[test]
fn concurrent_preparation_owners_cannot_alias_live_ranges() {
    let fixture = Fixture::new(64 * 1024);
    std::thread::scope(|scope| {
        for owner in 1..=4 {
            let arena = &fixture.arena;
            scope.spawn(move || {
                let layout = Layout::from_size_align(256, 64).unwrap();
                for _ in 0..1_000 {
                    let pointer = unsafe { arena.alloc(layout) };
                    assert!(!pointer.is_null());
                    unsafe { core::ptr::write_bytes(pointer, owner, layout.size()) };
                    std::thread::yield_now();
                    unsafe {
                        assert!(
                            core::slice::from_raw_parts(pointer, layout.size())
                                .iter()
                                .all(|byte| *byte == owner)
                        );
                        arena.dealloc(pointer, layout);
                    }
                }
            });
        }
    });
    assert_eq!(fixture.arena.live_bytes(), 0);
}

#[test]
fn resized_and_zeroed_objects_preserve_the_global_allocator_contract() {
    let fixture = Fixture::new(4096);
    let arena = &fixture.arena;
    let small = Layout::from_size_align(33, 128).unwrap();
    unsafe {
        let original = arena.alloc(small);
        assert!(!original.is_null());
        core::ptr::write_bytes(original, 0x93, small.size());
        let grown = arena.realloc(original, small, 257);
        assert!(!grown.is_null());
        assert_eq!(grown as usize % small.align(), 0);
        assert!(
            core::slice::from_raw_parts(grown, small.size())
                .iter()
                .all(|byte| *byte == 0x93)
        );
        let large = Layout::from_size_align(257, 128).unwrap();
        assert!(arena.realloc(grown, large, 8192).is_null());
        assert!(
            core::slice::from_raw_parts(grown, small.size())
                .iter()
                .all(|byte| *byte == 0x93)
        );
        arena.dealloc(grown, large);
        let zeroed = arena.alloc_zeroed(large);
        assert!(!zeroed.is_null());
        assert!(
            core::slice::from_raw_parts(zeroed, large.size())
                .iter()
                .all(|byte| *byte == 0)
        );
        arena.dealloc(zeroed, large);
    }
    assert_eq!(arena.live_bytes(), 0);
}
