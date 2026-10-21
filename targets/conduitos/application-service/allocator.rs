//! Guest-private bitmap allocator. Every dynamic byte belongs to admitted metadata.
use conduitos_application_core::memory::*;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};
const UNIT: usize = 32;
const UNITS: usize = HEAP_BYTES / UNIT;
const CURSOR: usize = METADATA + 8192;
// A failed allocation is terminal for this instance. Root quarantines every
// Capacity refusal; only installation of a fresh cleared grant resets the latch.
const EXHAUSTED: usize = CURSOR + core::mem::size_of::<usize>();
const BITMAP: usize = METADATA + 12288;
const _: () = assert!(EXHAUSTED + core::mem::size_of::<usize>() <= BITMAP);
const _: () = assert!(12288 + UNITS.div_ceil(64) * 8 <= HEADER_BYTES);
struct PrivateHeap;
#[global_allocator]
static HEAP: PrivateHeap = PrivateHeap;
pub fn exhausted() -> bool {
    // SAFETY: the exclusive Application metadata grant is installed before entry.
    unsafe { (EXHAUSTED as *const usize).read() != 0 }
}
fn capacity_failure() -> *mut u8 {
    // SAFETY: disjoint header word in the same exclusive metadata grant.
    unsafe { (EXHAUSTED as *mut usize).write(1) };
    ptr::null_mut()
}
fn rounded(layout: Layout) -> Option<usize> {
    layout
        .size()
        .max(1)
        .checked_add(UNIT - 1)
        .map(|n| n / UNIT)
        .filter(|&n| n <= UNITS)
}
unsafe fn occupied(index: usize) -> bool {
    unsafe { ((BITMAP as *const u64).add(index / 64).read() & (1u64 << (index % 64))) != 0 }
}
unsafe fn mark(start: usize, end: usize, used: bool) {
    for index in start..end {
        let pointer = unsafe { (BITMAP as *mut u64).add(index / 64) };
        let mask = 1u64 << (index % 64);
        let value = unsafe { pointer.read() };
        unsafe {
            pointer.write(if used { value | mask } else { value & !mask });
        }
    }
}
unsafe fn find(mut index: usize, end: usize, needed: usize, alignment: usize) -> Option<usize> {
    while index.checked_add(needed).is_some_and(|n| n <= end) {
        let address = (HEAP_START + index * UNIT).checked_add(alignment - 1)? & !(alignment - 1);
        index = (address - HEAP_START) / UNIT;
        if index.checked_add(needed).is_none_or(|n| n > end) {
            return None;
        }
        let mut cursor = index;
        while cursor < index + needed && !unsafe { occupied(cursor) } {
            cursor += 1;
        }
        if cursor == index + needed {
            return Some(index);
        }
        index = cursor + 1;
    }
    None
}
// SAFETY: Root admits one exclusive private metadata grant per service instance.
// Allocation bitmap/cursor live in its header, disjoint from State and heap.
// Each guest entry is exclusive; an interrupted/faulted entry is never resumed
// concurrently and Root clears the complete grant before replacement.
unsafe impl GlobalAlloc for PrivateHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if exhausted() {
            return ptr::null_mut();
        }
        let Some(needed) = rounded(layout) else {
            return capacity_failure();
        };
        let cursor = unsafe { (CURSOR as *const usize).read() };
        if cursor > UNITS {
            return capacity_failure();
        }
        let alignment = layout.align().max(UNIT);
        let found = unsafe { find(cursor, UNITS, needed, alignment) }
            .or_else(|| unsafe { find(0, cursor, needed, alignment) });
        let Some(index) = found else {
            return capacity_failure();
        };
        unsafe {
            mark(index, index + needed, true);
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
            mark(index, index + needed, false);
            let cursor = (CURSOR as *const usize).read();
            (CURSOR as *mut usize).write(cursor.min(index));
        }
    }
}
