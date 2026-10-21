//! Pure readiness fence for one synchronous Root idle lease, not completion.
pub(super) fn may_idle(
    epoch: u64,
    active: u64,
    before: u32,
    now: u32,
    elapsed: Option<i64>,
) -> bool {
    epoch != 0
        && active == epoch
        && before == now
        && elapsed.is_some_and(|ticks| (0..8).contains(&ticks))
}

#[cfg(test)]
mod tests {
    use super::may_idle;
    #[test]
    fn lease_epoch_and_absolute_deadline_are_both_required() {
        assert!(may_idle(7, 7, 10, 10, Some(0)));
        assert!(!may_idle(0, 0, 10, 10, Some(0)));
        assert!(!may_idle(7, 8, 10, 10, Some(0)));
        assert!(!may_idle(7, 0, 10, 10, Some(0)));
        for elapsed in [None, Some(-1), Some(8), Some(9)] {
            assert!(!may_idle(7, 7, 10, 10, elapsed));
        }
    }
    #[test]
    fn late_duplicate_or_new_rtc_fact_returns_root_without_completing_input() {
        for now in [0, 9, 11, u32::MAX] {
            assert!(!may_idle(7, 7, 10, now, Some(1)));
        }
        // A fact from an old lease cannot authorize a replacement lease.
        assert!(!may_idle(7, 8, 10, 10, Some(1)));
        // Even total loss of device IRQ cannot extend the absolute idle bound.
        assert!(!may_idle(7, 7, 10, 10, Some(8)));
    }
}
