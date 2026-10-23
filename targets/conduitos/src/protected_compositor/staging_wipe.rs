//! Erase only initialized staging words before releasing their allocation.
//! Volatile stores remain observable even when the allocation is then freed.
pub(super) fn clear(words: &mut [u32]) {
    for word in words {
        // SAFETY: each pointer is aligned, initialized and exclusively borrowed
        // for this store. No spare capacity or freed allocation is accessed.
        unsafe { core::ptr::write_volatile(word, 0) };
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn erases_every_initialized_word_without_touching_neighboring_storage() {
        let mut words = [0xdead_beef, 1, u32::MAX, 0x00ab_cdef, 0x1234_5678];
        clear(&mut words[1..4]);
        assert_eq!(words, [0xdead_beef, 0, 0, 0, 0x1234_5678]);
        clear(&mut words[1..1]);
        assert_eq!(words, [0xdead_beef, 0, 0, 0, 0x1234_5678]);
    }
}
