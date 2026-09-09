//! Allocation metadata never resides in released user storage.
//!
//! One bit per 32-byte unit covers at most 16 MiB, using a fixed 64 KiB bitmap.
//! A search inspects only this finite admitted range; it skips occupied runs.
//! Releasing adjacent allocations naturally makes their combined range usable.

use super::{ArenaError, MAXIMUM_ARENA_BYTES};
use core::{alloc::Layout, ptr::null_mut};

const UNIT: usize = 32;
const WORD_BITS: usize = 64;
const WORDS: usize = MAXIMUM_ARENA_BYTES / UNIT / WORD_BITS;

pub(super) struct ArenaState {
    start: usize,
    units: usize,
    live_units: usize,
    peak_units: usize,
    cursor: usize,
    allocated: [u64; WORDS],
    pub(super) sealed: bool,
}

impl ArenaState {
    pub(super) const fn new() -> Self {
        Self {
            start: 0,
            units: 0,
            live_units: 0,
            peak_units: 0,
            cursor: 0,
            allocated: [0; WORDS],
            sealed: false,
        }
    }

    pub(super) fn initialize(&mut self, start: usize, length: usize) -> Result<(), ArenaError> {
        let end = start.checked_add(length).ok_or(ArenaError::InvalidRange)?;
        let aligned = align_up(start, UNIT).ok_or(ArenaError::InvalidRange)?;
        let units = end.saturating_sub(aligned) / UNIT;
        if self.start != 0
            || self.sealed
            || start == 0
            || length > MAXIMUM_ARENA_BYTES
            || units == 0
        {
            return Err(ArenaError::InvalidRange);
        }
        self.start = aligned;
        self.units = units;
        Ok(())
    }

    pub(super) const fn used(&self) -> usize {
        self.peak_units * UNIT
    }
    pub(super) const fn live_bytes(&self) -> usize {
        self.live_units * UNIT
    }
    pub(super) const fn capacity(&self) -> usize {
        self.units * UNIT
    }

    pub(super) fn allocate(&mut self, layout: Layout) -> *mut u8 {
        if self.sealed || self.units == 0 {
            return null_mut();
        }
        let Some(needed) = allocation_units(layout) else {
            return null_mut();
        };
        if needed > self.units - self.live_units {
            return null_mut();
        }
        let alignment = layout.align().max(UNIT);
        let found = self
            .find_run(self.cursor, self.units, needed, alignment)
            .or_else(|| self.find_run(0, self.cursor, needed, alignment));
        let Some(index) = found else {
            return null_mut();
        };
        self.mark(index, index + needed, true);
        self.live_units += needed;
        self.peak_units = self.peak_units.max(self.live_units);
        self.cursor = index + needed;
        (self.start + index * UNIT) as *mut u8
    }

    pub(super) fn release(&mut self, pointer: *mut u8, layout: Layout) {
        let Some(offset) = (pointer as usize).checked_sub(self.start) else {
            return;
        };
        let Some(needed) = allocation_units(layout) else {
            return;
        };
        let index = offset / UNIT;
        if offset % UNIT != 0 || index >= self.units || needed > self.units - index {
            return;
        }
        // GlobalAlloc requires the exact live allocation and original Layout.
        // This changes metadata only; it never reads or writes released bytes.
        self.mark(index, index + needed, false);
        self.live_units -= needed;
        self.cursor = self.cursor.min(index);
    }

    fn find_run(
        &self,
        begin: usize,
        end_start: usize,
        needed: usize,
        alignment: usize,
    ) -> Option<usize> {
        let mut candidate = begin;
        while candidate < end_start {
            let address = align_up(self.start + candidate * UNIT, alignment)?;
            candidate = (address - self.start) / UNIT;
            if candidate >= end_start || candidate > self.units || needed > self.units - candidate {
                return None;
            }
            let end = candidate + needed;
            match self.first_occupied(candidate, end) {
                None => return Some(candidate),
                Some(occupied) => candidate = occupied + 1,
            }
        }
        None
    }

    fn first_occupied(&self, begin: usize, end: usize) -> Option<usize> {
        for word in begin / WORD_BITS..=(end - 1) / WORD_BITS {
            let bits = self.allocated[word] & range_mask(word, begin, end);
            if bits != 0 {
                return Some(word * WORD_BITS + bits.trailing_zeros() as usize);
            }
        }
        None
    }

    fn mark(&mut self, begin: usize, end: usize, allocated: bool) {
        for word in begin / WORD_BITS..=(end - 1) / WORD_BITS {
            let mask = range_mask(word, begin, end);
            if allocated {
                self.allocated[word] |= mask;
            } else {
                self.allocated[word] &= !mask;
            }
        }
    }
}

fn allocation_units(layout: Layout) -> Option<usize> {
    layout
        .size()
        .max(1)
        .checked_add(UNIT - 1)
        .map(|bytes| bytes / UNIT)
}

fn align_up(address: usize, alignment: usize) -> Option<usize> {
    address
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
}

fn range_mask(word: usize, begin: usize, end: usize) -> u64 {
    let base = word * WORD_BITS;
    let low = begin.saturating_sub(base);
    let high = (end - base).min(WORD_BITS);
    let below_high = if high == WORD_BITS {
        u64::MAX
    } else {
        (1_u64 << high) - 1
    };
    (u64::MAX << low) & below_high
}
