//! Exact first-fit bitmap operations; work advances by words, not heap bytes.
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

fn first_occupied(bitmap: &[u64], start: usize, end: usize) -> Option<usize> {
    let last = (end - 1) / 64;
    for (word_index, word) in bitmap.iter().enumerate().take(last + 1).skip(start / 64) {
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
        let occupied = word & mask(low, high);
        if occupied != 0 {
            return Some(word_index * 64 + occupied.trailing_zeros() as usize);
        }
    }
    None
}

pub fn find(
    bitmap: &[u64],
    heap_start: usize,
    mut index: usize,
    end: usize,
    needed: usize,
    alignment: usize,
) -> Option<usize> {
    if needed == 0 || !alignment.is_power_of_two() || end > bitmap.len().checked_mul(64)? {
        return None;
    }
    while index.checked_add(needed).is_some_and(|n| n <= end) {
        let address = heap_start
            .checked_add(index.checked_mul(UNIT)?)?
            .checked_add(alignment - 1)?
            & !(alignment - 1);
        index = address.checked_sub(heap_start)? / UNIT;
        let limit = index.checked_add(needed).filter(|&n| n <= end)?;
        match first_occupied(bitmap, index, limit) {
            None => return Some(index),
            Some(occupied) => index = occupied + 1,
        }
    }
    None
}

#[cfg(test)]
#[path = "bitmap_tests.rs"]
mod tests;
