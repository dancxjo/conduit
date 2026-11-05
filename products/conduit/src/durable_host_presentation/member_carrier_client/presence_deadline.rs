//! Client receipt accounting, not an owner authorization or a new session.
//! Expiry differences stay within the owner's clock; local bounds only narrow.
use super::{Duration, Instant, Out, PROTOCOL};

const MAXIMUM_LEASE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PresenceDeadline {
    pub(super) sequence: u64,
    pub(super) renew_at: Instant,
    pub(super) deadline: Instant,
    owner_expiry: u64,
}

impl PresenceDeadline {
    pub(super) fn initial(anchor: Instant) -> Self {
        Self {
            sequence: 0,
            renew_at: anchor,
            deadline: anchor + MAXIMUM_LEASE,
            owner_expiry: 0,
        }
    }

    pub(super) fn accept(
        &mut self,
        frame: &Out,
        requested: u64,
        anchor: Instant,
        overall: Instant,
        now: Instant,
    ) -> Result<(), &'static str> {
        let Out::PresenceAccepted {
            protocol: PROTOCOL,
            sequence,
            renew_after_millis,
            expires_at_millis,
        } = frame
        else {
            return Err("member lease acknowledgment schema differs");
        };
        if self.sequence.checked_add(1) != Some(requested)
            || *sequence != requested
            || !(1..=500).contains(renew_after_millis)
            || *expires_at_millis == 0
            || *expires_at_millis < self.owner_expiry
        {
            return Err("member lease acknowledgment sequence or bounds differ");
        }
        // An accepted receipt cannot revive a lease that expired in flight.
        if anchor > now || now >= self.deadline.min(overall) {
            return Err("member lease acknowledgment arrived too late");
        }
        let mut deadline = anchor
            .checked_add(MAXIMUM_LEASE)
            .ok_or("member lease deadline overflow")?
            .min(overall);
        if self.owner_expiry != 0 {
            // A capped expiry can repeat. Advance the receipt sequence, without
            // manufacturing another two seconds from its later arrival time.
            let delta =
                Duration::from_millis(*expires_at_millis - self.owner_expiry).min(MAXIMUM_LEASE);
            deadline = deadline.min(
                self.deadline
                    .checked_add(delta)
                    .ok_or("member lease deadline overflow")?,
            );
        }
        if now >= deadline {
            return Err("member lease acknowledgment arrived too late");
        }
        let renew_at = anchor
            .checked_add(Duration::from_millis(*renew_after_millis))
            .ok_or("member lease renewal overflow")?
            .min(deadline);
        *self = Self {
            sequence: *sequence,
            renew_at,
            deadline,
            owner_expiry: *expires_at_millis,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted(sequence: u64, expiry: u64) -> Out {
        Out::PresenceAccepted {
            protocol: PROTOCOL,
            sequence,
            renew_after_millis: 500,
            expires_at_millis: expiry,
        }
    }

    #[test]
    fn repeated_capped_expiry_advances_sequence_without_extending_deadline() {
        let start = Instant::now();
        let overall = start + Duration::from_secs(60);
        let mut state = PresenceDeadline::initial(start);
        state
            .accept(&accepted(1, 100_000), 1, start, overall, start)
            .unwrap();
        let original = state.deadline;
        for sequence in 2..=4 {
            let now = start + Duration::from_millis((sequence - 1) * 500);
            state
                .accept(&accepted(sequence, 100_000), sequence, now, overall, now)
                .unwrap();
            assert_eq!(state.sequence, sequence);
            assert_eq!(state.deadline, original);
        }
        let before = state;
        assert!(state
            .accept(&accepted(5, 100_000), 5, original, overall, original)
            .is_err());
        assert_eq!(state, before);
    }

    #[test]
    fn small_owner_expiry_advance_cannot_create_a_full_new_local_lease() {
        let start = Instant::now();
        let overall = start + Duration::from_secs(60);
        let mut state = PresenceDeadline::initial(start);
        state
            .accept(&accepted(1, 100_000), 1, start, overall, start)
            .unwrap();
        let previous = state.deadline;
        let now = start + Duration::from_millis(500);
        state
            .accept(&accepted(2, 100_010), 2, now, overall, now)
            .unwrap();
        assert_eq!(state.deadline, previous + Duration::from_millis(10));
    }

    #[test]
    fn invalid_or_late_receipts_never_change_retained_timing() {
        let start = Instant::now();
        let overall = start + Duration::from_millis(900);
        let mut state = PresenceDeadline::initial(start);
        state
            .accept(&accepted(1, 100_000), 1, start, overall, start)
            .unwrap();
        assert_eq!(state.deadline, overall);
        let before = state;
        for frame in [accepted(1, 101_000), accepted(2, 99_999), accepted(2, 0)] {
            assert!(state.accept(&frame, 2, start, overall, start).is_err());
            assert_eq!(state, before);
        }
        assert!(state
            .accept(&accepted(2, 101_000), 2, start, overall, overall)
            .is_err());
        assert_eq!(state, before);
    }
}
