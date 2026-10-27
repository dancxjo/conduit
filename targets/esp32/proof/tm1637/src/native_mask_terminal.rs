//! A pure preflight, never a retirement witness or a grant for another Play.
//! Actual kernel, renderer, issuer and evidence owners must still be consumed.
pub(crate) const CHILDREN: u64 = 960 * 60_000 / 3_000;
pub(crate) struct Completion {
    pub completed: u64,
    pub issued: u64,
    pub accepted: u64,
    pub first_sequence: u64,
    pub end_sequence: u64,
    pub next_sequence: u64,
    pub accepted_last: Option<u64>,
    pub revoked: bool,
    pub lease_pending: bool,
    pub first_lease_retained: bool,
    pub closed_pending: bool,
    pub call_retained: bool,
    pub renderer_retained: bool,
    pub faulted: bool,
    pub cleanup_refused: bool,
    pub normally_drained: bool,
}
impl Completion {
    pub(crate) fn ready(&self) -> bool {
        self.completed == CHILDREN
            && self.issued == CHILDREN
            && self.accepted == CHILDREN
            && self.first_sequence.checked_add(CHILDREN) == Some(self.end_sequence)
            && self.next_sequence == self.end_sequence
            && self.end_sequence.checked_sub(1) == self.accepted_last
            && self.revoked
            && !self.lease_pending
            && !self.first_lease_retained
            && !self.closed_pending
            && !self.call_retained
            && self.renderer_retained
            && !self.faulted
            && !self.cleanup_refused
            && self.normally_drained
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn completed() -> Completion {
        Completion {
            completed: CHILDREN,
            issued: CHILDREN,
            accepted: CHILDREN,
            first_sequence: 7,
            end_sequence: 7 + CHILDREN,
            next_sequence: 7 + CHILDREN,
            accepted_last: Some(6 + CHILDREN),
            revoked: true,
            lease_pending: false,
            first_lease_retained: false,
            closed_pending: false,
            call_retained: false,
            renderer_retained: true,
            faulted: false,
            cleanup_refused: false,
            normally_drained: true,
        }
    }
    #[test]
    fn exact_finite_completion_requires_each_distinct_custodian() {
        assert!(completed().ready());
        for field in 0..13 {
            let mut value = completed();
            match field {
                0 => value.completed -= 1,
                1 => value.issued -= 1,
                2 => value.accepted -= 1,
                3 => value.revoked = false,
                4 => value.lease_pending = true,
                5 => value.first_lease_retained = true,
                6 => value.closed_pending = true,
                7 => value.call_retained = true,
                8 => value.renderer_retained = false,
                9 => value.faulted = true,
                10 => value.cleanup_refused = true,
                11 => value.normally_drained = false,
                _ => value.accepted_last = None,
            }
            assert!(!value.ready(), "missing completion fact {field}");
        }
    }
    #[test]
    fn counts_and_sequence_arithmetic_never_slide_or_wrap() {
        for field in 0..5 {
            let mut value = completed();
            match field {
                0 => value.completed += 1,
                1 => value.issued += 1,
                2 => value.accepted += 1,
                3 => value.next_sequence -= 1,
                _ => value.accepted_last = Some(value.end_sequence),
            }
            assert!(!value.ready());
        }
        let mut value = completed();
        value.first_sequence = u64::MAX - 1;
        assert!(!value.ready());
    }
}
