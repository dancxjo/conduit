//! CPU proof of actual physical guard lifetimes, not ESP allocator geometry.
use super::*;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_network_workspace_reserve.rs"]
mod floor;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_mask_workspace.rs"]
mod workspace;
use core::{alloc::Layout, cell::Cell};
struct GuardAllocator<'a> {
    live: &'a Cell<usize>,
    deny: bool,
}
// SAFETY: accepted layouts are delegated unchanged to the global allocator;
// deallocation uses the exact pointer/layout and tracks only successful bytes.
unsafe impl floor::Allocator for GuardAllocator<'_> {
    unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        if self.deny {
            return core::ptr::null_mut();
        }
        let pointer = unsafe { std::alloc::alloc(layout) };
        if !pointer.is_null() {
            self.live.set(self.live.get() + layout.size());
        }
        pointer
    }
    unsafe fn deallocate(&mut self, pointer: *mut u8, layout: Layout) {
        self.live.set(self.live.get() - layout.size());
        unsafe { std::alloc::dealloc(pointer, layout) }
    }
}
#[test]
fn actual_floor_is_reproved_before_admission_and_held_through_original_kernel_install() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let gate = workspace::Gate::new();
    let request = gate.request().unwrap();
    let pause = gate.retired_boundary(true).unwrap();
    let permit = request.claim().ok().unwrap();
    let live = Cell::new(0);
    let stage = Cell::new(0);
    let mut initial_allocator = GuardAllocator {
        live: &live,
        deny: false,
    };
    let mut final_allocator = GuardAllocator {
        live: &live,
        deny: false,
    };
    let initial = floor::reserve(&mut initial_allocator).unwrap();
    assert_eq!(live.get(), 16384);
    let mut final_guard = None;
    let mut at_admission = None;
    let ((result, pointer), memory) = allocation_probe::observe(|| {
        let backing = NativePlay::reserve_linked_kernel_backing().unwrap();
        let pointer = backing.as_ptr() as usize;
        let result = play.install_linked_phased_held_with_boundaries(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            Some(backing),
            || {
                assert!(permit.held() && pause.waiting());
                assert_eq!(stage.replace(1), 0);
                drop(initial);
                assert_eq!(live.get(), 0);
                Ok(())
            },
            || {
                assert!(permit.held() && pause.waiting());
                assert_eq!(stage.replace(2), 1);
                final_guard = Some(floor::reserve(&mut final_allocator).unwrap());
                assert_eq!(live.get(), 16384);
                at_admission = allocation_probe::current_observation();
                Ok(())
            },
        );
        (result, pointer)
    });
    result.unwrap();
    assert_eq!(stage.get(), 2);
    assert!(permit.held() && pause.waiting());
    assert_eq!(
        live.get(),
        16384,
        "actual final guard survives kernel installation"
    );
    assert_eq!(
        pointer,
        play.prepared.as_ref().unwrap().as_ref() as *const _ as usize
    );
    let admitted = at_admission.unwrap();
    assert_eq!(
        (admitted.allocations, admitted.reallocations),
        (memory.allocations, memory.reallocations)
    );
    assert_eq!(play.phase, "installed-held");
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
    drop(final_guard);
    assert_eq!(live.get(), 0);
    drop(permit);
    assert!(!pause.waiting());
}
#[test]
fn refused_final_floor_or_revoked_permit_returns_original_ready_pair_and_unspent_issuer() {
    for revoked in [false, true] {
        let image = source_image();
        let pins = image.identity().unwrap();
        let mut issuer = issuer();
        let (mut source, mut join) = ready();
        let source_identity = source.as_ref().unwrap().native_identity().clone();
        let join_identity = join.as_ref().unwrap().native_identity().clone();
        let mut play = NativePlay::new();
        let gate = workspace::Gate::new();
        let request = gate.request().unwrap();
        let _pause = gate.retired_boundary(true).unwrap();
        let permit = request.claim().ok().unwrap();
        let live = Cell::new(0);
        let mut initial_allocator = GuardAllocator {
            live: &live,
            deny: false,
        };
        let mut final_allocator = GuardAllocator {
            live: &live,
            deny: true,
        };
        let initial = floor::reserve(&mut initial_allocator).unwrap();
        let admitted = Cell::new(false);
        let (result, memory) = allocation_probe::observe(|| {
            let backing = NativePlay::reserve_linked_kernel_backing().unwrap();
            play.install_linked_phased_held_with_boundaries(
                &image,
                &pins,
                c::OfferGeneration(u64::MAX),
                &mut source,
                &mut join,
                &mut issuer,
                Some(backing),
                || {
                    assert!(permit.held());
                    drop(initial);
                    if revoked {
                        gate.cancel();
                    }
                    Ok(())
                },
                || {
                    if !permit.held() {
                        return Err(());
                    }
                    let _guard = floor::reserve(&mut final_allocator).map_err(|_| ())?;
                    admitted.set(true);
                    Ok(())
                },
            )
        });
        assert!(result.is_err());
        assert!(!admitted.get());
        // Initial guard was allocated before observation and retired within it.
        assert_eq!(memory.allocations + 1, memory.deallocations);
        assert_eq!(live.get(), 0);
        assert_eq!(source.as_ref().unwrap().native_identity(), &source_identity);
        assert_eq!(join.as_ref().unwrap().native_identity(), &join_identity);
        assert!(!play.attempted && play.active.is_none() && play.flat_refusal.is_empty());
        assert!(play.prepared.is_none());
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || Ok(()),
        )
        .unwrap();
        assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
    }
}
