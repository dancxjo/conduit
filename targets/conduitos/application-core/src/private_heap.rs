//! Guest-private bitmap allocator. Every dynamic byte belongs to admitted metadata.
use crate::memory::*;
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
/// Allocator for an installed, exclusive, zero-initialized Application metadata grant.
/// Calling allocation or exhaustion observation requires that fixed grant to be
/// mapped; Root owns installation and clears it only between service instances.
pub struct PrivateHeap;
/// # Safety
/// The complete exclusive Application metadata grant must be mapped and remain
/// installed for the duration of this read.
pub unsafe fn exhausted() -> bool {
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
// Visit only the words intersecting [start, end), preserving neighboring bits.
fn word_mask(start: usize, end: usize, word: usize) -> u64 {
    let first = start.saturating_sub(word * 64);
    let last = (end - word * 64).min(64);
    (u64::MAX << first) & (u64::MAX >> (64 - last))
}
// SAFETY: callers provide start <= end within the installed bitmap extent.
unsafe fn mark_in(bitmap: *mut u64, start: usize, end: usize, used: bool) {
    for word in start / 64..end.div_ceil(64) {
        let mask = word_mask(start, end, word);
        let pointer = unsafe { bitmap.add(word) };
        // Fully covered words need no read of their old contents.
        let value = if mask == u64::MAX {
            0
        } else {
            unsafe { pointer.read() }
        };
        unsafe { pointer.write(if used { value | mask } else { value & !mask }) };
    }
}
unsafe fn mark(start: usize, end: usize, used: bool) {
    unsafe { mark_in(BITMAP as *mut u64, start, end, used) }
}
// Return the first matching unit as a bit scan would, skipping opposite words.
// SAFETY: callers provide start <= end within the installed bitmap extent.
unsafe fn first_matching(
    bitmap: *const u64,
    start: usize,
    end: usize,
    used: bool,
) -> Option<usize> {
    for word in start / 64..end.div_ceil(64) {
        let value = unsafe { bitmap.add(word).read() };
        let occupied = (if used { value } else { !value }) & word_mask(start, end, word);
        if occupied != 0 {
            return Some(word * 64 + occupied.trailing_zeros() as usize);
        }
    }
    None
}
unsafe fn find_in(
    bitmap: *const u64,
    mut index: usize,
    end: usize,
    needed: usize,
    alignment: usize,
) -> Option<usize> {
    while index.checked_add(needed).is_some_and(|n| n <= end) {
        let address = (HEAP_START + index * UNIT).checked_add(alignment - 1)? & !(alignment - 1);
        index = (address - HEAP_START) / UNIT;
        if index.checked_add(needed).is_none_or(|n| n > end) {
            return None;
        }
        match unsafe { first_matching(bitmap, index, index + needed, true) } {
            None => return Some(index),
            Some(cursor) => {
                // No candidate starting inside this occupied run can fit.
                index = unsafe { first_matching(bitmap, cursor + 1, end, false) }?;
            }
        }
    }
    None
}
// SAFETY: Root admits one exclusive private metadata grant per service instance.
// Allocation bitmap/cursor live in its header, disjoint from State and heap.
// Each guest entry is exclusive; an interrupted/faulted entry is never resumed
// concurrently and Root clears the complete grant before replacement.
unsafe impl GlobalAlloc for PrivateHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if unsafe { exhausted() } {
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
        let found = unsafe { find_in(BITMAP as *const u64, cursor, UNITS, needed, alignment) }
            .or_else(|| unsafe { find_in(BITMAP as *const u64, 0, cursor, needed, alignment) });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_ranges_match_bit_reference_and_preserve_neighbors() {
        for start in 0..193 {
            for end in start..=193 {
                for used in [false, true] {
                    let mut words = [0xa55a_aa55_1234_fedc; 4];
                    let mut expected = words;
                    for index in start..end {
                        let mask = 1 << (index % 64);
                        if used {
                            expected[index / 64] |= mask;
                        } else {
                            expected[index / 64] &= !mask;
                        }
                    }
                    unsafe { mark_in(words.as_mut_ptr(), start, end, used) };
                    assert_eq!(words, expected, "range {start}..{end} used={used}");
                }
            }
        }
    }

    #[test]
    fn occupancy_scan_matches_fragmented_bit_reference() {
        for words in [
            [0; 4],
            [u64::MAX; 4],
            [0, 1, 1 << 63, 0],
            [0xa55a_aa55_1234_fedc; 4],
        ] {
            for start in 0..256 {
                for end in start..=256 {
                    for used in [false, true] {
                        let expected = (start..end)
                            .find(|index| (words[index / 64] & (1 << (index % 64)) != 0) == used);
                        assert_eq!(
                            unsafe { first_matching(words.as_ptr(), start, end, used) },
                            expected
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn aligned_first_fit_matches_naive_fragmented_search() {
        for words in [
            [0; 4],
            [u64::MAX; 4],
            [u64::MAX, 0, u64::MAX, 0],
            [0xa55a_aa55_1234_fedc; 4],
        ] {
            for start in [0, 1, 63, 64, 65, 127] {
                for end in [128, 193, 256] {
                    for needed in [1, 2, 31, 63, 64, 65, 129] {
                        for alignment in [32, 64, 128, 4096] {
                            let expected = (start..end).find(|&index| {
                                (HEAP_START + index * UNIT).is_multiple_of(alignment)
                                    && index + needed <= end
                                    && (index..index + needed)
                                        .all(|at| words[at / 64] & (1 << (at % 64)) == 0)
                            });
                            assert_eq!(
                                unsafe { find_in(words.as_ptr(), start, end, needed, alignment) },
                                expected
                            );
                        }
                    }
                }
            }
        }
    }
}
