//! Finite lifecycle shared by ConduitOS timer-derived implementations.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerToken {
    play_generation: u32,
    request_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerCompletion {
    Current,
    Stale,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerError {
    AlreadyArmed,
    CapacityExhausted,
    StalePlayGeneration,
}

/// One admitted timer slot with a finite request budget.
/// Tokens are local to this owner, not bearer capabilities. Root must separately
/// establish fresh identity and authority when replacing the owner itself.
/// The admitted slot cannot be duplicated through Copy or Clone.
/// ```compile_fail,E0382
/// use conduitos::timer_nucleus::TimerNucleus;
/// let mut timer = TimerNucleus::new(1, 1);
/// let moved = timer;
/// let _ = timer.arm();
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct TimerNucleus {
    play_generation: u32,
    next_request: u32,
    remaining_requests: u16,
    pending: Option<TimerToken>,
    cancelled: Option<TimerToken>,
}

impl TimerNucleus {
    pub const fn new(play_generation: u32, request_capacity: u16) -> Self {
        Self {
            play_generation,
            next_request: 0,
            remaining_requests: request_capacity,
            pending: None,
            cancelled: None,
        }
    }

    pub fn arm(&mut self) -> Result<TimerToken, TimerError> {
        if self.pending.is_some() {
            return Err(TimerError::AlreadyArmed);
        }
        if self.remaining_requests == 0 {
            return Err(TimerError::CapacityExhausted);
        }
        // At most u16::MAX requests are admitted between replans, starting at
        // zero. This addition cannot reach u32 exhaustion in a valid owner.
        self.next_request += 1;
        let token = TimerToken {
            play_generation: self.play_generation,
            request_generation: self.next_request,
        };
        self.remaining_requests -= 1;
        self.pending = Some(token);
        Ok(token)
    }

    pub fn cancel(&mut self) -> Option<TimerToken> {
        let token = self.pending.take()?;
        self.cancelled = Some(token);
        Some(token)
    }

    pub fn complete(&mut self, token: TimerToken) -> TimerCompletion {
        if self.cancelled == Some(token) {
            self.cancelled = None;
            return TimerCompletion::Cancelled;
        }
        if self.pending == Some(token) {
            self.pending = None;
            TimerCompletion::Current
        } else {
            TimerCompletion::Stale
        }
    }

    /// Replace this owner's Play only with a strictly newer local generation.
    /// Refusal leaves pending/cancelled tokens and remaining capacity intact.
    /// At u32::MAX this owner cannot replan again; wrapping is not renewal.
    /// This local generation neither preserves nor grants physical authority.
    pub fn replan(
        &mut self,
        play_generation: u32,
        request_capacity: u16,
    ) -> Result<(), TimerError> {
        if play_generation <= self.play_generation {
            return Err(TimerError::StalePlayGeneration);
        }
        self.play_generation = play_generation;
        self.next_request = 0;
        self.remaining_requests = request_capacity;
        self.pending = None;
        self.cancelled = None;
        Ok(())
    }

    pub const fn remaining_requests(&self) -> u16 {
        self.remaining_requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_arms_and_distinguishes_completion_classes() {
        let mut timers = TimerNucleus::new(7, 2);
        let first = timers.arm().unwrap();
        assert_eq!(timers.arm(), Err(TimerError::AlreadyArmed));
        assert_eq!(timers.cancel(), Some(first));
        assert_eq!(timers.complete(first), TimerCompletion::Cancelled);

        let second = timers.arm().unwrap();
        assert_eq!(timers.complete(second), TimerCompletion::Current);
        assert_eq!(timers.complete(second), TimerCompletion::Stale);
        assert_eq!(timers.arm(), Err(TimerError::CapacityExhausted));
    }

    #[test]
    fn replan_rejects_old_play_completion_and_installs_fresh_capacity() {
        let mut timers = TimerNucleus::new(1, 1);
        let old = timers.arm().unwrap();
        timers.replan(2, 1).unwrap();
        assert_eq!(timers.complete(old), TimerCompletion::Stale);
        let fresh = timers.arm().unwrap();
        assert_ne!(fresh, old);
        assert_eq!(timers.complete(fresh), TimerCompletion::Current);
    }

    #[test]
    fn same_or_older_replan_cannot_recreate_a_live_token() {
        let mut timers = TimerNucleus::new(7, 1);
        let old = timers.arm().unwrap();
        for generation in [7, 6, 0] {
            assert_eq!(
                timers.replan(generation, u16::MAX),
                Err(TimerError::StalePlayGeneration)
            );
            assert_eq!(timers.remaining_requests(), 0);
            assert_eq!(timers.arm(), Err(TimerError::AlreadyArmed));
        }
        timers.replan(8, 1).unwrap();
        let fresh = timers.arm().unwrap();
        assert_ne!(old, fresh);
        assert_eq!(timers.complete(old), TimerCompletion::Stale);
        assert_eq!(timers.arm(), Err(TimerError::AlreadyArmed));
        assert_eq!(timers.complete(fresh), TimerCompletion::Current);
        assert_eq!(timers.complete(fresh), TimerCompletion::Stale);
    }

    #[test]
    fn refused_replan_preserves_cancelled_custody_and_exhausted_capacity() {
        let mut timers = TimerNucleus::new(9, 1);
        let old = timers.arm().unwrap();
        assert_eq!(timers.cancel(), Some(old));
        assert_eq!(timers.replan(9, 1), Err(TimerError::StalePlayGeneration));
        assert_eq!(timers.complete(old), TimerCompletion::Cancelled);
        assert_eq!(timers.complete(old), TimerCompletion::Stale);
        assert_eq!(timers.arm(), Err(TimerError::CapacityExhausted));
        timers.replan(10, 1).unwrap();
        let fresh = timers.arm().unwrap();
        assert_eq!(timers.complete(old), TimerCompletion::Stale);
        assert_eq!(timers.complete(fresh), TimerCompletion::Current);
    }

    #[test]
    fn maximum_play_generation_is_terminal_without_retiring_current_request() {
        let mut timers = TimerNucleus::new(u32::MAX - 1, 1);
        let old = timers.arm().unwrap();
        timers.replan(u32::MAX, 1).unwrap();
        let last = timers.arm().unwrap();
        for generation in [u32::MAX, 0, 1, u32::MAX - 1] {
            assert_eq!(
                timers.replan(generation, 1),
                Err(TimerError::StalePlayGeneration)
            );
        }
        assert_eq!(timers.complete(old), TimerCompletion::Stale);
        assert_eq!(timers.complete(last), TimerCompletion::Current);
        assert_eq!(timers.arm(), Err(TimerError::CapacityExhausted));
    }

    #[test]
    fn maximum_request_capacity_never_repeats_a_token_or_renews_on_refusal() {
        let mut timers = TimerNucleus::new(0, u16::MAX);
        let first = timers.arm().unwrap();
        assert_eq!(timers.complete(first), TimerCompletion::Current);
        for _ in 1..u16::MAX {
            let current = timers.arm().unwrap();
            assert_ne!(first, current);
            assert_eq!(timers.complete(first), TimerCompletion::Stale);
            assert_eq!(timers.complete(current), TimerCompletion::Current);
        }
        assert_eq!(timers.arm(), Err(TimerError::CapacityExhausted));
        assert_eq!(timers.replan(0, 1), Err(TimerError::StalePlayGeneration));
        assert_eq!(timers.arm(), Err(TimerError::CapacityExhausted));
        timers.replan(1, 0).unwrap();
        assert_eq!(timers.arm(), Err(TimerError::CapacityExhausted));
    }
}
