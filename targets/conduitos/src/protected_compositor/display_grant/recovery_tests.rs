//! Host allocator model for the exact contiguous staging admission failure.
use super::*;
use crate::allocation::BootArena;
use core::alloc::{GlobalAlloc, Layout};

#[test]
fn fresh_identity_allocations_fragment_released_staging_but_shared_custody_does_not() {
    // Static metadata keeps the actual allocator's fixed bitmap off test stack.
    static ARENA: BootArena = BootArena::new();
    let mut backing = std::vec![0u8; 16 * 1024 * 1024 + 32];
    let start = (backing.as_mut_ptr() as usize + 31) & !31;
    // SAFETY: this test alone owns the backing range for every arena allocation.
    unsafe { ARENA.initialize(start, 16 * 1024 * 1024).unwrap() };
    let identity = Layout::from_size_align(32, 8).unwrap();
    let staging = Layout::from_size_align(8 * 1024 * 1024, 4).unwrap();
    let metadata = Layout::from_size_align(1024, 8).unwrap();
    // No simulated allocation is dereferenced. Exact Layouts are retained until
    // release, and backing lives until all custom allocations have been freed.
    unsafe {
        let first_identities: [*mut u8; 4] = core::array::from_fn(|_| ARENA.alloc(identity));
        assert!(first_identities.iter().all(|pointer| !pointer.is_null()));
        let first = ARENA.alloc(staging);
        assert!(!first.is_null());
        let owner = ARENA.alloc(metadata);
        assert!(!owner.is_null());
        ARENA.dealloc(first, staging);
        let fresh_identities: [*mut u8; 4] = core::array::from_fn(|_| ARENA.alloc(identity));
        assert!(fresh_identities.iter().all(|pointer| !pointer.is_null()));
        assert!(ARENA.alloc(staging).is_null());
        assert_eq!(ARENA.capacity(), 16 * 1024 * 1024);
        assert_eq!(ARENA.live_bytes(), 1280);
        for pointer in fresh_identities {
            ARENA.dealloc(pointer, identity);
        }
        // Reusing immutable grant custody introduces no new blocks before this
        // reservation. The real allocator admits the identical released range.
        let replacement = ARENA.alloc(staging);
        assert_eq!(replacement, first);
        ARENA.dealloc(replacement, staging);
        ARENA.dealloc(owner, metadata);
        for pointer in first_identities {
            ARENA.dealloc(pointer, identity);
        }
        assert_eq!(ARENA.live_bytes(), 0);
    }
}

#[test]
fn shared_grant_retains_exact_scope_without_refreshing_replaced_provider() {
    let host = HostId::from("host");
    let boot = BootId::from("boot");
    let base = HostBaseId::from("display");
    let format = DisplayFormat {
        width: 3840,
        height: 2160,
        pitch: 3840 * 4,
        bits_per_pixel: 32,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
    };
    let grant: SharedDisplayGrant = Rc::new(
        DisplayGrant::new(
            host.clone(),
            boot.clone(),
            OfferGeneration(3),
            base.clone(),
            format,
        )
        .unwrap(),
    );
    let retained = grant.clone();
    assert!(Rc::ptr_eq(&retained, &grant));
    assert!(retained.matches(&host, &boot, OfferGeneration(3), &base));
    assert!(retained.format_matches(format));
    for (host, boot, generation, base) in [
        (
            HostId::from("other"),
            boot.clone(),
            OfferGeneration(3),
            base.clone(),
        ),
        (
            host.clone(),
            BootId::from("new"),
            OfferGeneration(3),
            base.clone(),
        ),
        (host.clone(), boot.clone(), OfferGeneration(4), base.clone()),
        (
            host,
            boot,
            OfferGeneration(3),
            HostBaseId::from("replacement"),
        ),
    ] {
        assert!(!retained.matches(&host, &boot, generation, &base));
    }
    assert!(!retained.format_matches(DisplayFormat {
        width: 3839,
        ..format
    }));
}
