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
}

/// One admitted timer slot with a finite request budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        self.next_request = self.next_request.wrapping_add(1);
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

    pub fn replan(&mut self, play_generation: u32, request_capacity: u16) {
        self.play_generation = play_generation;
        self.next_request = 0;
        self.remaining_requests = request_capacity;
        self.pending = None;
        self.cancelled = None;
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
        timers.replan(2, 1);
        assert_eq!(timers.complete(old), TimerCompletion::Stale);
        let fresh = timers.arm().unwrap();
        assert_ne!(fresh, old);
        assert_eq!(timers.complete(fresh), TimerCompletion::Current);
    }
}
