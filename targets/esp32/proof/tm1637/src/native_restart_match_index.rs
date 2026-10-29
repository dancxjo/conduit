//! Derived private history index: four low-two-bit classes, 704 bytes total.
//! This filters candidates only; the matcher still checks complete bytes and
//! preserves nearest-first ties. All ring positions use 32-bit bitmap words.
use super::V4_HISTORY;
// Production ownership is solely inside Encoder. Its existing Drop and reset
// clear this derived private state; no separate aggregate Drop is necessary.
pub(in super::super) struct Index {
    bits: [[u32; 44]; 4],
}
impl Index {
    pub(in super::super) const fn new() -> Self {
        Self { bits: [[0; 44]; 4] }
    }
    pub(in super::super) fn bucket(&self, byte: u8) -> &[u32; 44] {
        &self.bits[usize::from(byte & 3)]
    }
    pub(in super::super) fn observe(&mut self, position: usize, byte: u8) {
        let word = position / 32;
        let mask = 1u32 << (position % 32);
        for bucket in &mut self.bits {
            bucket[word] &= !mask;
        }
        self.bits[usize::from(byte & 3)][word] |= mask;
    }
    pub(in super::super) fn clear(&mut self) {
        // The bitmap reveals derived private byte classes. Volatile typed stores
        // follow the encoder's existing private-byte wipe without aliasing casts.
        for bucket in &mut self.bits {
            for word in bucket {
                unsafe { core::ptr::write_volatile(word, 0) };
            }
        }
    }
}
pub(super) struct Candidates<'a> {
    bits: &'a [u32; 44],
    at: usize,
    remaining: usize,
    distance: usize,
}
impl<'a> Candidates<'a> {
    pub(super) fn new(bits: &'a [u32; 44], consumed: usize, maximum: usize, first: usize) -> Self {
        Self {
            bits,
            at: ((consumed % V4_HISTORY) + V4_HISTORY - first) % V4_HISTORY,
            remaining: maximum.saturating_sub(first - 1),
            distance: first,
        }
    }
}
impl Iterator for Candidates<'_> {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        while self.remaining > 0 {
            let width = (self.at % 32 + 1).min(self.remaining);
            let low = self.at % 32 + 1 - width;
            let upper = u32::MAX >> (31 - self.at % 32);
            let mask = upper & (u32::MAX << low);
            let found = self.bits[self.at / 32] & mask;
            if found != 0 {
                let bit = 31 - found.leading_zeros() as usize;
                let skip = self.at % 32 - bit;
                let at = self.at - skip;
                let distance = self.distance + skip;
                self.remaining -= skip + 1;
                self.distance += skip + 1;
                self.at = if at == 0 { V4_HISTORY - 1 } else { at - 1 };
                return Some((distance, at));
            }
            self.remaining -= width;
            self.distance += width;
            self.at = if self.at + 1 == width {
                V4_HISTORY - 1
            } else {
                self.at - width
            };
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_ring_order_overwrite_and_private_clear() {
        let mut index = Index::new();
        assert_eq!(core::mem::size_of_val(&index), 704);
        for at in 0..V4_HISTORY {
            index.observe(at, (at % 4) as u8);
        }
        for consumed in [
            1408,
            1409,
            1439,
            1440,
            2815,
            2816,
            usize::MAX - 1,
            usize::MAX,
        ] {
            for byte in 0..4 {
                let actual: Vec<_> =
                    Candidates::new(index.bucket(byte), consumed, V4_HISTORY, 1).collect();
                let expected: Vec<_> = (1..=V4_HISTORY)
                    .filter_map(|d| {
                        let at = ((consumed % V4_HISTORY) + V4_HISTORY - d) % V4_HISTORY;
                        (at % 4 == usize::from(byte)).then_some((d, at))
                    })
                    .collect();
                assert_eq!(actual, expected);
            }
        }
        index.observe(0, 3);
        assert_eq!(index.bucket(0)[0] & 1, 0);
        assert_eq!(index.bucket(3)[0] & 1, 1);
        index.clear();
        assert!(index.bits.iter().flatten().all(|word| *word == 0));
    }
}
