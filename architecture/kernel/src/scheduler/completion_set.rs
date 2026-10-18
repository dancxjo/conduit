//! Monotonic terminal membership within the existing N-byte reservation.
//! Packed flags leave a native-word tail for the count when N is at least
//! twice the word width in bytes. Smaller sets inspect at most two flag bytes
//! on supported targets; neither path scans an active-node prefix.
pub(super) struct CompletionSet<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> CompletionSet<N> {
    pub(super) fn new() -> Self {
        Self { bytes: [0; N] }
    }

    pub(super) fn contains(&self, node: usize) -> bool {
        assert!(node < N);
        self.bytes[node / 8] & (1 << (node % 8)) != 0
    }

    pub(super) fn count(&self) -> usize {
        if N >= 2 * core::mem::size_of::<usize>() {
            usize::from_le_bytes(
                self.bytes[N - core::mem::size_of::<usize>()..]
                    .try_into()
                    .unwrap(),
            )
        } else {
            self.bytes[..N.div_ceil(8)]
                .iter()
                .map(|byte| byte.count_ones() as usize)
                .sum()
        }
    }

    /// Returns whether this is the first terminal transition for the node.
    pub(super) fn mark(&mut self, node: usize) -> bool {
        if self.contains(node) {
            return false;
        }
        let count = self.count() + 1;
        self.bytes[node / 8] |= 1 << (node % 8);
        if N >= 2 * core::mem::size_of::<usize>() {
            self.bytes[N - core::mem::size_of::<usize>()..].copy_from_slice(&count.to_le_bytes());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay<const N: usize>() {
        assert_eq!(core::mem::size_of::<CompletionSet<N>>(), N);
        assert_eq!(core::mem::align_of::<CompletionSet<N>>(), 1);
        for active in 0..=N {
            let mut set = CompletionSet::<N>::new();
            assert_eq!(set.count(), 0);
            for node in (0..active).rev() {
                assert!(!set.contains(node));
                assert!(set.mark(node));
                assert!(!set.mark(node));
                assert_eq!(set.count(), active - node);
            }
            for node in 0..N {
                assert_eq!(set.contains(node), node < active);
            }
        }
    }

    #[test]
    fn partial_prefixes_and_duplicate_retirement_keep_exact_membership() {
        replay::<0>();
        replay::<1>();
        replay::<7>();
        replay::<8>();
        replay::<9>();
        replay::<15>();
        replay::<16>();
        replay::<17>();
        replay::<63>();
        replay::<64>();
        replay::<65>();
        replay::<257>();
    }

    #[test]
    fn count_represents_the_entire_admitted_node_cardinality() {
        let mut set = CompletionSet::<65_536>::new();
        for node in 0..65_536 {
            assert!(set.mark(node));
        }
        assert_eq!(set.count(), 65_536);
        assert!(set.contains(65_535));
        assert!(!set.mark(65_535));
        assert_eq!(set.count(), 65_536);
    }
}
