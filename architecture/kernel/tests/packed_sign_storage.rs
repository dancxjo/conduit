//! Actual heap backing may shrink without enlarging the canonical Sign quota.
#![cfg(feature = "alloc")]
use conduit_kernel::{HostedSignLog, KernelEvent, KernelEventKind, NodeId, SignError, SignSink};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    mem::size_of,
};
thread_local! {
 static TRACK:Cell<bool>=const{Cell::new(false)};
 static BYTES:Cell<usize>=const{Cell::new(0)};
}
struct Allocator;
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: pass the exact layout to the backing allocator.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && TRACK.try_with(Cell::get).unwrap_or(false) {
            BYTES.with(|n| n.set(n.get() + layout.size()));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: pass the live allocation and original layout to System.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[test]
fn compact_physical_backing_preserves_full_canonical_quota() {
    let items = 128;
    let quota = u32::from(items) * size_of::<KernelEvent>() as u32;
    BYTES.with(|n| n.set(0));
    TRACK.with(|v| v.set(true));
    let mut log = HostedSignLog::new(items, quota).unwrap();
    TRACK.with(|v| v.set(false));
    let physical = BYTES.with(Cell::get);
    assert_eq!(
        Some(physical),
        HostedSignLog::canonical_storage_bytes(items, 0)
    );
    assert_eq!(physical, log.reserved_storage_bytes());
    assert!(physical < usize::from(items) * size_of::<Option<KernelEvent>>());
    assert_eq!(log.item_capacity(), items);
    assert_eq!(log.byte_capacity(), quota);
    for _ in 0..items {
        log.record(NodeId(0), None, None, KernelEventKind::BackCompleted)
            .unwrap();
    }
    assert_eq!(log.used_bytes(), quota);
    assert_eq!(
        log.record(NodeId(0), None, None, KernelEventKind::BackCompleted),
        Err(SignError::ItemCapacityExceeded)
    );
    assert_eq!(log.len(), items);
}
