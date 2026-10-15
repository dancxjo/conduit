//! Explicit diagnostic legacy enumeration; shares the actual scheduler's
//! eligibility, wake state, Step, storage and Sign machinery.
pub(super) struct LinearCandidates {
    cursor: usize,
    active: usize,
    offset: usize,
}
impl LinearCandidates {
    pub(super) fn new(cursor: usize, active: usize) -> Self {
        Self {
            cursor,
            active,
            offset: 0,
        }
    }
    #[cfg(test)]
    pub(super) fn take_byte_probes(&mut self) -> usize {
        0
    }
}
impl Iterator for LinearCandidates {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        if self.offset == self.active {
            return None;
        }
        let node = (self.cursor + self.offset) % self.active;
        self.offset += 1;
        Some(node)
    }
}
