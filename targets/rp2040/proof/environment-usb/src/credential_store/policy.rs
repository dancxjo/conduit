//! Clock/profile predicates, never flash possession or proof of quiescence.
pub(super) const BUDGET_MS: u32 = 750;
const BUDGET_US: u64 = BUDGET_MS as u64 * 1000;
pub(super) fn reviewed_profile(jedec: Option<u32>, clock_hz: u32) -> bool {
    jedec == Some(0xef4015) && clock_hz == 125_000_000
}
pub(super) fn can_start(now_us: u64, wait_deadline_us: u64, request_deadline_us: u64) -> bool {
    now_us
        .checked_add(BUDGET_US)
        .is_some_and(|end| end <= wait_deadline_us && end <= request_deadline_us)
}
pub(super) fn completed(
    start_us: u64,
    now_us: u64,
    wait_deadline_us: u64,
    request_deadline_us: u64,
) -> bool {
    now_us
        .checked_sub(start_us)
        .is_some_and(|duration| duration <= BUDGET_US)
        && now_us <= wait_deadline_us
        && now_us <= request_deadline_us
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_true_slack_is_required_even_if_millisecond_gate_rounds_down() {
        assert!(can_start(0, 750_000, 750_000));
        assert!(!can_start(1, 750_000, 750_000));
        assert!(!can_start(0, 750_000, 749_999));
        assert!(!can_start(0, 749_999, 750_000));
        assert!(!can_start(u64::MAX, u64::MAX, u64::MAX));
    }
    #[test]
    fn overrun_refuses_even_when_original_wait_has_more_slack() {
        assert!(completed(10, 750_010, 2_000_000, 6_000_000));
        assert!(!completed(10, 750_011, 2_000_000, 6_000_000));
        assert!(!completed(10, 800_010, 2_000_000, 6_000_000));
        assert!(!completed(10, 9, 2_000_000, 6_000_000));
        assert!(!completed(10, 20, 19, 6_000_000));
        assert!(!completed(10, 20, 2_000_000, 19));
    }
    #[test]
    fn unsupported_profile_or_clock_has_no_write_envelope() {
        assert!(reviewed_profile(Some(0xef4015), 125_000_000));
        for jedec in [
            None,
            Some(0),
            Some(0xffffff),
            Some(0xef7015),
            Some(0xef4016),
        ] {
            assert!(!reviewed_profile(jedec, 125_000_000));
        }
        assert!(!reviewed_profile(Some(0xef4015), 124_999_999));
        assert!(!reviewed_profile(Some(0xef4015), 250_000_000));
    }
}
