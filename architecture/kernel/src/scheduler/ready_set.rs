//! Fixed packed readiness with cursor-ordered iteration; no Play allocation.
//! Retains the previous N-byte storage ceiling without generic-const arithmetic.
pub(super) struct ReadySet<const N: usize> {
    bytes: [u8; N],
}
impl<const N: usize> ReadySet<N> {
    pub(super) fn new(active: usize) -> Self {
        let mut ready = Self { bytes: [0; N] };
        for node in 0..active {
            ready.set(node, true);
        }
        ready
    }
    pub(super) fn set(&mut self, node: usize, value: bool) {
        let byte = node / 8;
        let previous = self.bytes[byte];
        let mask = 1 << (node % 8);
        if value {
            self.bytes[byte] |= mask;
        } else {
            self.bytes[byte] &= !mask;
        }
        if N >= 64 && (previous == 0) != (self.bytes[byte] == 0) {
            // The existing N-byte reservation has room after the packed bits.
            // One summary bit marks each nonempty readiness byte.
            let summary = N.div_ceil(8) + byte / 8;
            let mask = 1 << (byte % 8);
            if self.bytes[byte] != 0 {
                self.bytes[summary] |= mask;
            } else {
                self.bytes[summary] &= !mask;
            }
        }
    }
    pub(super) fn clear(&mut self) {
        self.bytes.fill(0);
    }
    pub(super) fn contains(&self, node: usize) -> bool {
        self.bytes[node / 8] & (1 << (node % 8)) != 0
    }
    #[cfg(any(test, not(feature = "linear-ready-reference-test")))]
    fn next_between(&self, mut start: usize, end: usize) -> (Option<usize>, usize) {
        let mut probes = 0;
        while start < end {
            probes += 1;
            let byte = start / 8;
            let available = self.bytes[byte] & (u8::MAX << (start % 8));
            if available != 0 {
                let node = byte * 8 + available.trailing_zeros() as usize;
                return ((node < end).then_some(node), probes);
            }
            start = (byte + 1) * 8;
            if N >= 64 {
                while start < end {
                    let byte = start / 8;
                    probes += 1;
                    let summary = self.bytes[N.div_ceil(8) + byte / 8] & (u8::MAX << (byte % 8));
                    if summary != 0 {
                        start = (byte / 8 * 8 + summary.trailing_zeros() as usize) * 8;
                        break;
                    }
                    start = (byte / 8 + 1) * 64;
                }
            }
        }
        (None, probes)
    }
    #[cfg(feature = "linear-ready-reference-test")]
    pub(super) fn scheduler_candidates(
        &self,
        cursor: usize,
        active: usize,
    ) -> super::ready_reference::LinearCandidates {
        super::ready_reference::LinearCandidates::new(cursor, active)
    }
    #[cfg(not(feature = "linear-ready-reference-test"))]
    pub(super) fn scheduler_candidates(&self, cursor: usize, active: usize) -> ReadyNodes<'_, N> {
        self.round_robin(cursor, active)
    }
    #[cfg(any(test, not(feature = "linear-ready-reference-test")))]
    pub(super) fn round_robin(&self, cursor: usize, active: usize) -> ReadyNodes<'_, N> {
        ReadyNodes {
            ready: self,
            cursor,
            active,
            next: cursor,
            wrapped: false,
            #[cfg(test)]
            byte_probes: 0,
        }
    }
}
#[cfg(any(test, not(feature = "linear-ready-reference-test")))]
pub(super) struct ReadyNodes<'a, const N: usize> {
    ready: &'a ReadySet<N>,
    cursor: usize,
    active: usize,
    next: usize,
    wrapped: bool,
    #[cfg(test)]
    byte_probes: usize,
}
#[cfg(any(test, not(feature = "linear-ready-reference-test")))]
impl<const N: usize> ReadyNodes<'_, N> {
    #[cfg(test)]
    #[cfg(not(feature = "linear-ready-reference-test"))]
    pub(super) fn take_byte_probes(&mut self) -> usize {
        core::mem::take(&mut self.byte_probes)
    }
}
#[cfg(any(test, not(feature = "linear-ready-reference-test")))]
impl<const N: usize> Iterator for ReadyNodes<'_, N> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        if !self.wrapped {
            let (node, _probes) = self.ready.next_between(self.next, self.active);
            #[cfg(test)]
            {
                self.byte_probes += _probes;
            }
            if let Some(node) = node {
                self.next = node + 1;
                return Some(node);
            }
            self.wrapped = true;
            self.next = 0;
        }
        let (node, _probes) = self.ready.next_between(self.next, self.cursor);
        #[cfg(test)]
        {
            self.byte_probes += _probes;
        }
        let node = node?;
        self.next = node + 1;
        Some(node)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    #[test]
    fn packed_iteration_matches_linear_round_robin_at_every_cursor() {
        for active in 0..=17 {
            let mut set = ReadySet::<17>::new(active);
            for mask in 0u32..512 {
                for node in 0..active {
                    set.set(node, mask & (1 << (node % 9)) != 0);
                }
                for node in 0..active {
                    assert_eq!(set.contains(node), mask & (1 << (node % 9)) != 0);
                }
                for cursor in 0..active.max(1) {
                    let expected = (0..active)
                        .map(|offset| (cursor + offset) % active)
                        .filter(|&node| mask & (1 << (node % 9)) != 0)
                        .collect::<std::vec::Vec<_>>();
                    assert_eq!(
                        set.round_robin(cursor, active)
                            .collect::<std::vec::Vec<_>>(),
                        expected
                    );
                }
            }
        }
        assert_eq!(core::mem::size_of::<ReadySet<17>>(), 17);
    }
    #[test]
    fn summary_iteration_matches_linear_for_partial_active_prefixes() {
        for active in [0, 1, 7, 8, 63, 64, 65, 127, 128, 129] {
            for mask in 0u32..64 {
                let mut set = ReadySet::<129>::new(active);
                for node in 0..active {
                    set.set(node, mask & (1 << (node % 6)) != 0);
                }
                for cursor in 0..active.max(1) {
                    let expected = (0..active)
                        .map(|offset| (cursor + offset) % active)
                        .filter(|node| mask & (1 << (node % 6)) != 0)
                        .collect::<std::vec::Vec<_>>();
                    assert_eq!(
                        set.round_robin(cursor, active)
                            .collect::<std::vec::Vec<_>>(),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn sparse_large_selection_skips_empty_groups_without_growing_storage() {
        let mut set = ReadySet::<10_000>::new(0);
        set.set(9_999, true);
        let (node, probes) = set.next_between(0, 10_000);
        assert_eq!(node, Some(9_999));
        assert!(probes <= 160, "observed {probes} readiness byte probes");
        assert_eq!(core::mem::size_of_val(&set), 10_000);
        set.set(9_999, false);
        assert_eq!(set.round_robin(9_999, 10_000).next(), None);
        for node in [0, 7, 8, 63, 64, 511, 512, 9_998, 9_999] {
            set.set(node, true);
            set.set(node, true);
        }
        set.set(512, false);
        for cursor in [0, 1, 63, 64, 511, 512, 9_998, 9_999] {
            let expected = (0..10_000)
                .map(|offset| (cursor + offset) % 10_000)
                .filter(|node| set.contains(*node))
                .collect::<std::vec::Vec<_>>();
            assert_eq!(
                set.round_robin(cursor, 10_000)
                    .collect::<std::vec::Vec<_>>(),
                expected
            );
        }
        set.clear();
        assert_eq!(set.round_robin(0, 10_000).next(), None);
    }

    #[test]
    fn isolated_wake_clear_and_large_boundaries_preserve_independent_membership() {
        let mut set = ReadySet::<256>::new(256);
        set.clear();
        let mut expected = [false; 256];
        for node in [0, 7, 8, 63, 64, 127, 128, 254, 255] {
            set.set(node, true);
            set.set(node, true);
            expected[node] = true;
        }
        set.set(64, false);
        expected[64] = false;
        for (node, expected) in expected.iter().enumerate() {
            assert_eq!(set.contains(node), *expected);
        }
        for cursor in 0..256 {
            let linear = (0..256)
                .map(|offset| (cursor + offset) % 256)
                .filter(|&node| expected[node])
                .collect::<std::vec::Vec<_>>();
            assert_eq!(
                set.round_robin(cursor, 256).collect::<std::vec::Vec<_>>(),
                linear
            );
        }
        set.clear();
        assert!(set.round_robin(0, 256).next().is_none());
    }
}
