//! Guest-private bitmap allocator. Every dynamic byte belongs to admitted metadata.
use conduitos_creche_core::memory::*;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};
#[path = "allocator/bitmap.rs"]
mod bitmap;
use bitmap::UNIT;
const UNITS: usize = HEAP_BYTES / UNIT;
const CURSOR: usize = METADATA + 8192;
const BITMAP: usize = METADATA + 12288;
const _: () = assert!(12288 + UNITS.div_ceil(64) * 8 <= HEADER_BYTES);
struct PrivateHeap;
#[global_allocator]
static HEAP: PrivateHeap = PrivateHeap;
fn rounded(layout: Layout) -> Option<usize> {
    layout
        .size()
        .max(1)
        .checked_add(UNIT - 1)
        .map(|n| n / UNIT)
        .filter(|&n| n <= UNITS)
}
unsafe fn bitmap<'entry>() -> &'entry mut [u64] {
    // SAFETY: the caller holds Root's exclusive entry for the entire borrow;
    // the slice cannot escape alloc/dealloc or outlive that entry.
    unsafe { core::slice::from_raw_parts_mut(BITMAP as *mut u64, UNITS.div_ceil(64)) }
}
// SAFETY: Root admits one exclusive private metadata grant per service instance.
// Allocation bitmap/cursor live in its header, disjoint from State and heap.
// Each guest entry is exclusive; an interrupted/faulted entry is never resumed
// concurrently and Root clears the complete grant before replacement.
unsafe impl GlobalAlloc for PrivateHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(needed) = rounded(layout) else {
            return ptr::null_mut();
        };
        let cursor = unsafe { (CURSOR as *const usize).read() };
        if cursor > UNITS {
            return ptr::null_mut();
        }
        let alignment = layout.align().max(UNIT);
        let bitmap = unsafe { bitmap() };
        let found = bitmap::find(bitmap, HEAP_START, cursor, UNITS, needed, alignment)
            .or_else(|| bitmap::find(bitmap, HEAP_START, 0, cursor, needed, alignment));
        let Some(index) = found else {
            return ptr::null_mut();
        };
        unsafe {
            if !bitmap::mark(bitmap, index, index + needed, true) {
                return ptr::null_mut();
            }
            (CURSOR as *mut usize).write(index + needed);
        }
        (HEAP_START + index * UNIT) as *mut u8
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let Some(offset) = (pointer as usize).checked_sub(HEAP_START) else {
            return;
        };
        let Some(needed) = rounded(layout) else {
            return;
        };
        let index = offset / UNIT;
        if !offset.is_multiple_of(UNIT) || index.checked_add(needed).is_none_or(|n| n > UNITS) {
            return;
        }
        unsafe {
            if !bitmap::mark(bitmap(), index, index + needed, false) {
                return;
            }
            let cursor = (CURSOR as *const usize).read();
            (CURSOR as *mut usize).write(cursor.min(index));
        }
    }
}
