//! Exact first-fit bitmap operations with bounded word spans for large requests.
pub const UNIT: usize = 32;

fn mask(low: usize, high: usize) -> u64 {
    (u64::MAX << low) & (u64::MAX >> (64 - high))
}

pub fn mark(bitmap: &mut [u64], start: usize, end: usize, used: bool) -> bool {
    let Some(bits) = bitmap.len().checked_mul(64) else {
        return false;
    };
    if start > end || end > bits {
        return false;
    }
    if start == end {
        return true;
    }
    let last = (end - 1) / 64;
    for (word_index, word) in bitmap
        .iter_mut()
        .enumerate()
        .take(last + 1)
        .skip(start / 64)
    {
        let low = if word_index == start / 64 {
            start % 64
        } else {
            0
        };
        let high = if word_index == last {
            (end - 1) % 64 + 1
        } else {
            64
        };
        let selected = mask(low, high);
        *word = if used {
            *word | selected
        } else {
            *word & !selected
        };
    }
    true
}

fn aligned(heap_start: usize, index: usize, alignment: usize) -> Option<usize> {
    let address = heap_start
        .checked_add(index.checked_mul(UNIT)?)?
        .checked_add(alignment - 1)?
        & !(alignment - 1);
    Some(address.checked_sub(heap_start)? / UNIT)
}

fn small_find(
    bitmap: &[u64],
    heap_start: usize,
    mut index: usize,
    end: usize,
    needed: usize,
    alignment: usize,
) -> Option<usize> {
    while index.checked_add(needed).is_some_and(|n| n <= end) {
        let address = (heap_start + index * UNIT).checked_add(alignment - 1)? & !(alignment - 1);
        index = (address - heap_start) / UNIT;
        if index.checked_add(needed).is_none_or(|n| n > end) {
            return None;
        }
        let mut cursor = index;
        while cursor < index + needed && bitmap[cursor / 64] & (1 << (cursor % 64)) == 0 {
            cursor += 1;
        }
        if cursor == index + needed {
            return Some(index);
        }
        index = cursor + 1;
    }
    None
}

fn large_find(
    bitmap: &[u64],
    heap_start: usize,
    index: usize,
    end: usize,
    needed: usize,
    alignment: usize,
) -> Option<usize> {
    let mut run_start = index;
    for (word_index, word) in bitmap
        .iter()
        .enumerate()
        .take((end - 1) / 64 + 1)
        .skip(index / 64)
    {
        let base = word_index * 64;
        let begin = index.max(base);
        let stop = end.min(base + 64);
        let occupied = *word & mask(begin - base, stop - base);
        if occupied != 0 {
            let prefix_end = base + occupied.trailing_zeros() as usize;
            let candidate = aligned(heap_start, run_start, alignment)?;
            if candidate
                .checked_add(needed)
                .is_some_and(|n| n <= prefix_end)
            {
                return Some(candidate);
            }
            // Any internal free gap in this word is at most 63 units, so it
            // cannot fit this >64-unit request. Carry only the free suffix.
            run_start = base + 64 - occupied.leading_zeros() as usize;
        }
        let candidate = aligned(heap_start, run_start, alignment)?;
        if candidate.checked_add(needed).is_some_and(|n| n <= stop) {
            return Some(candidate);
        }
    }
    None
}

pub fn find(
    bitmap: &[u64],
    heap_start: usize,
    index: usize,
    end: usize,
    needed: usize,
    alignment: usize,
) -> Option<usize> {
    if needed == 0
        || !alignment.is_power_of_two()
        || end > bitmap.len().checked_mul(64)?
        || index.checked_add(needed).is_none_or(|n| n > end)
    {
        return None;
    }
    let _heap_end = heap_start.checked_add(end.checked_mul(UNIT)?)?;
    // Small requests retain the original cheap bit walk. Whole-word searching
    // per occupied bit makes heavily fragmented small requests more expensive.
    if needed <= 64 {
        small_find(bitmap, heap_start, index, end, needed, alignment)
    } else {
        large_find(bitmap, heap_start, index, end, needed, alignment)
    }
}
