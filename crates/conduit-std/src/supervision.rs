pub const SUPERVISION_MAX_ATTEMPTS: u16 = 8;
pub const SUPERVISION_MAX_OBSERVATIONS: usize = 16;
pub const SUPERVISION_MAX_DURATION_TICKS: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisionError {
    InvalidBounds,
    ClockReversed,
    DeadlineOverflow,
    DeadlineExpired,
    AttemptExhausted,
    RetryForbidden,
    ReplayAfterCommitForbidden,
    EntropyRequired,
    Terminal,
}

impl SupervisionError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidBounds => "CND-SVP-001",
            Self::ClockReversed => "CND-SVP-002",
            Self::DeadlineOverflow => "CND-SVP-003",
            Self::DeadlineExpired => "CND-SVP-004",
            Self::AttemptExhausted => "CND-SVP-005",
            Self::RetryForbidden => "CND-SVP-006",
            Self::ReplayAfterCommitForbidden => "CND-SVP-007",
            Self::EntropyRequired => "CND-SVP-008",
            Self::Terminal => "CND-SVP-009",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackoffMode {
    Fixed,
    Exponential,
}

/// Exact retry spacing. Jitter is deterministic only when the caller injects
/// an entropy word; the policy never reads ambient randomness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackoffPolicy {
    pub mode: BackoffMode,
    pub initial_ticks: u64,
    pub maximum_ticks: u64,
    pub jitter_ticks: u64,
}

impl BackoffPolicy {
    pub const fn validate(self) -> Result<(), SupervisionError> {
        if self.initial_ticks == 0
            || self.initial_ticks > self.maximum_ticks
            || self.maximum_ticks > SUPERVISION_MAX_DURATION_TICKS
            || self.jitter_ticks > self.maximum_ticks
        {
            return Err(SupervisionError::InvalidBounds);
        }
        Ok(())
    }

    pub fn delay(
        self,
        completed_attempt: u16,
        entropy: Option<u64>,
    ) -> Result<u64, SupervisionError> {
        self.validate()?;
        let base = match self.mode {
            BackoffMode::Fixed => self.initial_ticks,
            BackoffMode::Exponential => {
                let shifts = u32::from(completed_attempt.saturating_sub(1)).min(63);
                self.initial_ticks
                    .checked_shl(shifts)
                    .unwrap_or(u64::MAX)
                    .min(self.maximum_ticks)
            }
        };
        let jitter = if self.jitter_ticks == 0 {
            0
        } else {
            entropy.ok_or(SupervisionError::EntropyRequired)?
                % self
                    .jitter_ticks
                    .checked_add(1)
                    .ok_or(SupervisionError::InvalidBounds)?
        };
        base.checked_add(jitter)
            .map(|delay| delay.min(self.maximum_ticks))
            .ok_or(SupervisionError::DeadlineOverflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryPermission {
    Forbidden,
    Idempotent,
    ReconcileBeforeRetry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptOutcome {
    Succeeded,
    EligibleFailure,
    CommittedFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryDecision {
    Succeeded { attempt: u16 },
    Retry { attempt: u16, not_before_tick: u64 },
    Exhausted { attempts: u16 },
}

/// Bounded state for one operation against one immutable exact binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryState {
    maximum_attempts: u16,
    deadline_tick: u64,
    backoff: BackoffPolicy,
    permission: RetryPermission,
    committed_replay_permitted: bool,
    attempt: u16,
    next_not_before_tick: Option<u64>,
    last_tick: u64,
    generation: u32,
    cancelled: bool,
    terminal: bool,
}

impl RetryState {
    pub fn new(
        maximum_attempts: u16,
        deadline_tick: u64,
        backoff: BackoffPolicy,
        permission: RetryPermission,
        committed_replay_permitted: bool,
        generation: u32,
    ) -> Result<Self, SupervisionError> {
        backoff.validate()?;
        if maximum_attempts == 0
            || maximum_attempts > SUPERVISION_MAX_ATTEMPTS
            || deadline_tick == 0
        {
            return Err(SupervisionError::InvalidBounds);
        }
        Ok(Self {
            maximum_attempts,
            deadline_tick,
            backoff,
            permission,
            committed_replay_permitted,
            attempt: 1,
            next_not_before_tick: None,
            last_tick: 0,
            generation,
            cancelled: false,
            terminal: false,
        })
    }

    fn observe_tick(&mut self, now: u64) -> Result<(), SupervisionError> {
        if now < self.last_tick {
            return Err(SupervisionError::ClockReversed);
        }
        self.last_tick = now;
        if now >= self.deadline_tick {
            self.terminal = true;
            return Err(SupervisionError::DeadlineExpired);
        }
        if self.cancelled || self.terminal {
            return Err(SupervisionError::Terminal);
        }
        Ok(())
    }

    pub fn observe(
        &mut self,
        now: u64,
        outcome: AttemptOutcome,
        entropy: Option<u64>,
    ) -> Result<RetryDecision, SupervisionError> {
        self.observe_tick(now)?;
        if outcome == AttemptOutcome::Succeeded {
            self.terminal = true;
            return Ok(RetryDecision::Succeeded {
                attempt: self.attempt,
            });
        }
        if self.permission == RetryPermission::Forbidden {
            self.terminal = true;
            return Err(SupervisionError::RetryForbidden);
        }
        if outcome == AttemptOutcome::CommittedFailure
            && !(self.permission == RetryPermission::Idempotent && self.committed_replay_permitted)
        {
            self.terminal = true;
            return Err(SupervisionError::ReplayAfterCommitForbidden);
        }
        if self.attempt >= self.maximum_attempts {
            self.terminal = true;
            return Ok(RetryDecision::Exhausted {
                attempts: self.attempt,
            });
        }
        let delay = self.backoff.delay(self.attempt, entropy)?;
        let not_before_tick = now
            .checked_add(delay)
            .ok_or(SupervisionError::DeadlineOverflow)?;
        if not_before_tick >= self.deadline_tick {
            self.terminal = true;
            return Ok(RetryDecision::Exhausted {
                attempts: self.attempt,
            });
        }
        self.attempt = self
            .attempt
            .checked_add(1)
            .ok_or(SupervisionError::AttemptExhausted)?;
        self.next_not_before_tick = Some(not_before_tick);
        Ok(RetryDecision::Retry {
            attempt: self.attempt,
            not_before_tick,
        })
    }

    pub fn ready(&mut self, now: u64) -> Result<bool, SupervisionError> {
        self.observe_tick(now)?;
        let ready = self
            .next_not_before_tick
            .is_some_and(|not_before| now >= not_before);
        if ready {
            self.next_not_before_tick = None;
        }
        Ok(ready)
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.next_not_before_tick = None;
    }

    #[must_use]
    pub const fn attempt(&self) -> u16 {
        self.attempt
    }

    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub const fn next_not_before_tick(&self) -> Option<u64> {
        self.next_not_before_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakerState {
    Closed,
    Open { until_tick: u64 },
    HalfOpen { admitted_probes: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakerAdmission {
    Admitted,
    RejectedOpen { until_tick: u64 },
    RejectedProbeLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BreakerOutcome {
    Success,
    CountedFailure,
    IgnoredFailure,
}

/// Finite observation-window circuit breaker with deterministic transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CircuitBreakerState<const N: usize> {
    observations: [Option<bool>; N],
    len: usize,
    maximum_observations: usize,
    failure_threshold: usize,
    cooldown_ticks: u64,
    maximum_half_open_probes: u16,
    state: BreakerState,
    last_tick: u64,
    generation: u32,
    cancelled: bool,
}

impl<const N: usize> CircuitBreakerState<N> {
    pub fn new(
        maximum_observations: usize,
        failure_threshold: usize,
        cooldown_ticks: u64,
        maximum_half_open_probes: u16,
    ) -> Result<Self, SupervisionError> {
        if maximum_observations == 0
            || maximum_observations > N
            || maximum_observations > SUPERVISION_MAX_OBSERVATIONS
            || failure_threshold == 0
            || failure_threshold > maximum_observations
            || cooldown_ticks == 0
            || cooldown_ticks > SUPERVISION_MAX_DURATION_TICKS
            || maximum_half_open_probes == 0
        {
            return Err(SupervisionError::InvalidBounds);
        }
        Ok(Self {
            observations: [None; N],
            len: 0,
            maximum_observations,
            failure_threshold,
            cooldown_ticks,
            maximum_half_open_probes,
            state: BreakerState::Closed,
            last_tick: 0,
            generation: 0,
            cancelled: false,
        })
    }

    fn observe_tick(&mut self, now: u64) -> Result<(), SupervisionError> {
        if now < self.last_tick {
            return Err(SupervisionError::ClockReversed);
        }
        self.last_tick = now;
        if self.cancelled {
            return Err(SupervisionError::Terminal);
        }
        Ok(())
    }

    pub fn admit(&mut self, now: u64) -> Result<BreakerAdmission, SupervisionError> {
        self.observe_tick(now)?;
        if let BreakerState::Open { until_tick } = self.state {
            if now < until_tick {
                return Ok(BreakerAdmission::RejectedOpen { until_tick });
            }
            self.state = BreakerState::HalfOpen { admitted_probes: 0 };
        }
        match self.state {
            BreakerState::Closed => Ok(BreakerAdmission::Admitted),
            BreakerState::HalfOpen {
                ref mut admitted_probes,
            } if *admitted_probes < self.maximum_half_open_probes => {
                *admitted_probes += 1;
                Ok(BreakerAdmission::Admitted)
            }
            BreakerState::HalfOpen { .. } => Ok(BreakerAdmission::RejectedProbeLimit),
            BreakerState::Open { until_tick } => Ok(BreakerAdmission::RejectedOpen { until_tick }),
        }
    }

    pub fn observe(
        &mut self,
        now: u64,
        outcome: BreakerOutcome,
    ) -> Result<BreakerState, SupervisionError> {
        self.observe_tick(now)?;
        match (self.state, outcome) {
            (BreakerState::HalfOpen { .. }, BreakerOutcome::Success) => self.reset(),
            (BreakerState::HalfOpen { .. }, BreakerOutcome::CountedFailure) => {
                self.open(now)?;
            }
            (_, BreakerOutcome::IgnoredFailure) => {}
            (BreakerState::Closed, outcome) => {
                if self.len == self.maximum_observations {
                    self.observations.copy_within(1..self.len, 0);
                    self.len -= 1;
                }
                self.observations[self.len] = Some(outcome == BreakerOutcome::CountedFailure);
                self.len += 1;
                let failures = self.observations[..self.len]
                    .iter()
                    .filter(|value| **value == Some(true))
                    .count();
                if failures >= self.failure_threshold {
                    self.open(now)?;
                }
            }
            (BreakerState::Open { .. }, _) => {}
        }
        Ok(self.state)
    }

    fn open(&mut self, now: u64) -> Result<(), SupervisionError> {
        let until_tick = now
            .checked_add(self.cooldown_ticks)
            .ok_or(SupervisionError::DeadlineOverflow)?;
        self.state = BreakerState::Open { until_tick };
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(SupervisionError::InvalidBounds)?;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.observations = [None; N];
        self.len = 0;
        self.state = BreakerState::Closed;
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    #[must_use]
    pub const fn state(&self) -> BreakerState {
        self.state
    }

    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed() -> BackoffPolicy {
        BackoffPolicy {
            mode: BackoffMode::Fixed,
            initial_ticks: 2,
            maximum_ticks: 2,
            jitter_ticks: 0,
        }
    }

    #[test]
    fn retry_success_backoff_exhaustion_and_permissions_are_exact() {
        let mut retry =
            RetryState::new(3, 20, fixed(), RetryPermission::Idempotent, false, 7).unwrap();
        assert_eq!(
            retry.observe(1, AttemptOutcome::EligibleFailure, None),
            Ok(RetryDecision::Retry {
                attempt: 2,
                not_before_tick: 3
            })
        );
        assert_eq!(retry.ready(2), Ok(false));
        assert_eq!(retry.ready(3), Ok(true));
        assert_eq!(
            retry.observe(4, AttemptOutcome::EligibleFailure, None),
            Ok(RetryDecision::Retry {
                attempt: 3,
                not_before_tick: 6
            })
        );
        assert_eq!(
            retry.observe(6, AttemptOutcome::EligibleFailure, None),
            Ok(RetryDecision::Exhausted { attempts: 3 })
        );

        let mut forbidden =
            RetryState::new(2, 10, fixed(), RetryPermission::Forbidden, false, 0).unwrap();
        assert_eq!(
            forbidden.observe(1, AttemptOutcome::EligibleFailure, None),
            Err(SupervisionError::RetryForbidden)
        );
        let mut committed = RetryState::new(
            2,
            10,
            fixed(),
            RetryPermission::ReconcileBeforeRetry,
            false,
            0,
        )
        .unwrap();
        assert_eq!(
            committed.observe(1, AttemptOutcome::CommittedFailure, None),
            Err(SupervisionError::ReplayAfterCommitForbidden)
        );
    }

    #[test]
    fn exponential_cap_jitter_deadline_and_cancellation_are_explicit() {
        let policy = BackoffPolicy {
            mode: BackoffMode::Exponential,
            initial_ticks: 2,
            maximum_ticks: 7,
            jitter_ticks: 2,
        };
        assert_eq!(policy.delay(1, Some(5)), Ok(4));
        assert_eq!(policy.delay(3, Some(2)), Ok(7));
        assert_eq!(
            policy.delay(1, None),
            Err(SupervisionError::EntropyRequired)
        );
        let mut retry =
            RetryState::new(2, 5, fixed(), RetryPermission::Idempotent, false, 0).unwrap();
        assert_eq!(
            retry.observe(4, AttemptOutcome::EligibleFailure, None),
            Ok(RetryDecision::Exhausted { attempts: 1 })
        );
        let mut cancelled =
            RetryState::new(2, 10, fixed(), RetryPermission::Idempotent, false, 0).unwrap();
        cancelled.cancel();
        assert_eq!(cancelled.ready(1), Err(SupervisionError::Terminal));
    }

    #[test]
    fn breaker_window_open_half_open_probe_and_reset_are_deterministic() {
        let mut breaker = CircuitBreakerState::<4>::new(3, 2, 5, 1).unwrap();
        assert_eq!(breaker.admit(1), Ok(BreakerAdmission::Admitted));
        assert_eq!(
            breaker.observe(1, BreakerOutcome::CountedFailure),
            Ok(BreakerState::Closed)
        );
        assert_eq!(
            breaker.observe(2, BreakerOutcome::Success),
            Ok(BreakerState::Closed)
        );
        assert_eq!(
            breaker.observe(3, BreakerOutcome::CountedFailure),
            Ok(BreakerState::Open { until_tick: 8 })
        );
        assert_eq!(
            breaker.admit(7),
            Ok(BreakerAdmission::RejectedOpen { until_tick: 8 })
        );
        assert_eq!(breaker.admit(8), Ok(BreakerAdmission::Admitted));
        assert_eq!(breaker.admit(8), Ok(BreakerAdmission::RejectedProbeLimit));
        assert_eq!(
            breaker.observe(8, BreakerOutcome::Success),
            Ok(BreakerState::Closed)
        );
        assert_eq!(breaker.generation(), 1);
    }
}
