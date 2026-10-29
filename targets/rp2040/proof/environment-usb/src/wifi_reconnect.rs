//! Finite volatile carrier retry timing; no membership or routing policy.
pub(super) fn can_start(now: u64, deadline: u64) -> bool {
    now.checked_add(45_000).is_some_and(|end| end <= deadline)
}
pub(super) fn network_error(error: Option<&str>) -> bool {
    matches!(
        error,
        Some(
            "radio-leave-timeout"
                | "radio-join-timeout"
                | "radio-join-refused"
                | "radio-dhcp-timeout"
        )
    )
}
const MAX_DELAY_MS: u64 = 60_000;
#[derive(Default)]
pub(super) struct Reconnect {
    failures: u8,
    due: Option<u64>,
}
impl Reconnect {
    pub(super) fn completed(&mut self, now: u64, usable: bool) {
        if usable {
            self.failures = 0;
            self.due = None;
        } else {
            self.failures = self.failures.saturating_add(1);
            let delay = (5_000u64 << self.failures.saturating_sub(1).min(4)).min(MAX_DELAY_MS);
            self.due = Some(now.saturating_add(delay));
        }
    }
    pub(super) fn poll(&mut self, now: u64, usable: bool, link: bool) -> bool {
        if usable {
            self.completed(now, true);
            return false;
        }
        if self.due.is_none() {
            // Give a still-associated station a full DHCP grace period.
            self.due = Some(now.saturating_add(if link { MAX_DELAY_MS } else { 5_000 }));
        }
        if self.due.is_some_and(|due| now >= due) {
            self.due = None;
            return true;
        }
        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_requests_never_start_and_only_network_errors_clear() {
        assert!(can_start(5_000, 50_000));
        assert!(!can_start(5_001, 50_000));
        assert!(!can_start(u64::MAX, u64::MAX));
        assert!(network_error(Some("radio-dhcp-timeout")));
        assert!(!network_error(Some("attachment-generation-exhausted")));
    }
    #[test]
    fn failed_attempts_back_off_and_saturate() {
        let mut r = Reconnect::default();
        let mut now = 0;
        for delay in [5_000, 10_000, 20_000, 40_000, 60_000, 60_000] {
            r.completed(now, false);
            assert!(!r.poll(now + delay - 1, false, false));
            now += delay;
            assert!(r.poll(now, false, false));
        }
    }
    #[test]
    fn late_dhcp_cancels_retry_and_later_loss_starts_fresh() {
        let mut r = Reconnect::default();
        assert!(!r.poll(0, false, true));
        assert!(!r.poll(59_999, false, true));
        assert!(!r.poll(60_000, true, true));
        assert!(!r.poll(70_000, false, false));
        assert!(r.poll(75_000, false, false));
        r.completed(75_000, false);
        assert!(!r.poll(79_999, false, false));
        assert!(r.poll(80_000, false, false));
    }
}
