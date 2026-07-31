/// Maximum duration accepted by the portable time-family reference providers.
pub const TIME_MAX_DURATION_TICKS: u64 = 1_000_000;
/// Every current portable time-family provider retains at most one value.
pub const TIME_MAX_RETAINED_VALUES: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeError {
    DurationTooLarge,
    DeadlineOverflow,
    ClockReversed,
    RetainedValueBoundExceeded,
}

impl TimeError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DurationTooLarge => "CND-TIM-001",
            Self::DeadlineOverflow => "CND-TIM-002",
            Self::ClockReversed => "CND-TIM-003",
            Self::RetainedValueBoundExceeded => "CND-TIM-004",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalPendingPolicy {
    Flush,
    Drop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebounceMode {
    Leading,
    Trailing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThrottleMode {
    LeadingBlock,
    TrailingCoalesce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    EmitNow,
    RetainUntil(u64),
    BlockUntil(u64),
    CoalescedUntil(u64),
}

pub fn exact_deadline(now: u64, duration: u64) -> Result<u64, TimeError> {
    if duration > TIME_MAX_DURATION_TICKS {
        return Err(TimeError::DurationTooLarge);
    }
    now.checked_add(duration).ok_or(TimeError::DeadlineOverflow)
}

/// One bounded timer shared by the portable delay and timeout proofs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OneShotTimer {
    last_tick: u64,
    deadline: Option<u64>,
}

impl OneShotTimer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_tick: 0,
            deadline: None,
        }
    }

    pub fn arm(&mut self, now: u64, duration: u64) -> Result<u64, TimeError> {
        self.observe(now)?;
        let deadline = exact_deadline(now, duration)?;
        self.deadline = Some(deadline);
        Ok(deadline)
    }

    pub fn observe(&mut self, now: u64) -> Result<(), TimeError> {
        if now < self.last_tick {
            return Err(TimeError::ClockReversed);
        }
        self.last_tick = now;
        Ok(())
    }

    pub fn due(&mut self, now: u64) -> Result<bool, TimeError> {
        self.observe(now)?;
        let due = self.deadline.is_some_and(|deadline| now >= deadline);
        if due {
            self.deadline = None;
        }
        Ok(due)
    }

    #[must_use]
    pub const fn deadline(&self) -> Option<u64> {
        self.deadline
    }

    pub fn cancel(&mut self) {
        self.deadline = None;
    }
}

impl Default for OneShotTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Timing-only state for a one-value debounce node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebounceState {
    timer: OneShotTimer,
    retained: bool,
    leading_window: bool,
}

impl DebounceState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timer: OneShotTimer::new(),
            retained: false,
            leading_window: false,
        }
    }

    pub fn admit(
        &mut self,
        now: u64,
        duration: u64,
        mode: DebounceMode,
    ) -> Result<Admission, TimeError> {
        let deadline = self.timer.arm(now, duration)?;
        match mode {
            DebounceMode::Leading if !self.leading_window => {
                self.leading_window = true;
                Ok(Admission::EmitNow)
            }
            DebounceMode::Leading => Ok(Admission::BlockUntil(deadline)),
            DebounceMode::Trailing => {
                self.retained = true;
                Ok(Admission::CoalescedUntil(deadline))
            }
        }
    }

    pub fn poll(&mut self, now: u64, mode: DebounceMode) -> Result<bool, TimeError> {
        if !self.timer.due(now)? {
            return Ok(false);
        }
        match mode {
            DebounceMode::Leading => {
                self.leading_window = false;
                Ok(false)
            }
            DebounceMode::Trailing => Ok(core::mem::take(&mut self.retained)),
        }
    }

    pub fn finish(&mut self, policy: TerminalPendingPolicy) -> bool {
        self.timer.cancel();
        self.leading_window = false;
        let retained = core::mem::take(&mut self.retained);
        retained && matches!(policy, TerminalPendingPolicy::Flush)
    }

    pub fn cancel(&mut self) {
        self.timer.cancel();
        self.retained = false;
        self.leading_window = false;
    }
}

impl Default for DebounceState {
    fn default() -> Self {
        Self::new()
    }
}

/// Timing-only state for explicit leading-block or trailing-coalesce throttle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThrottleState {
    timer: OneShotTimer,
    retained: bool,
}

impl ThrottleState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timer: OneShotTimer::new(),
            retained: false,
        }
    }

    pub fn admit(
        &mut self,
        now: u64,
        duration: u64,
        mode: ThrottleMode,
    ) -> Result<Admission, TimeError> {
        if let Some(deadline) = self.timer.deadline()
            && now < deadline
        {
            return match mode {
                ThrottleMode::LeadingBlock => Ok(Admission::BlockUntil(deadline)),
                ThrottleMode::TrailingCoalesce => {
                    self.retained = true;
                    Ok(Admission::CoalescedUntil(deadline))
                }
            };
        }
        let deadline = self.timer.arm(now, duration)?;
        match mode {
            ThrottleMode::LeadingBlock => Ok(Admission::EmitNow),
            ThrottleMode::TrailingCoalesce => {
                self.retained = true;
                Ok(Admission::RetainUntil(deadline))
            }
        }
    }

    pub fn poll(&mut self, now: u64, mode: ThrottleMode) -> Result<bool, TimeError> {
        if !self.timer.due(now)? {
            return Ok(false);
        }
        Ok(matches!(mode, ThrottleMode::TrailingCoalesce) && core::mem::take(&mut self.retained))
    }

    pub fn finish(&mut self, policy: TerminalPendingPolicy) -> bool {
        self.timer.cancel();
        let retained = core::mem::take(&mut self.retained);
        retained && matches!(policy, TerminalPendingPolicy::Flush)
    }

    pub fn cancel(&mut self) {
        self.timer.cancel();
        self.retained = false;
    }
}

impl Default for ThrottleState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_timer_covers_zero_maximum_late_and_discontinuous_ticks() {
        let mut timer = OneShotTimer::new();
        assert_eq!(timer.arm(4, 0), Ok(4));
        assert_eq!(timer.due(4), Ok(true));
        assert_eq!(
            timer.arm(4, TIME_MAX_DURATION_TICKS),
            Ok(4 + TIME_MAX_DURATION_TICKS)
        );
        assert_eq!(timer.due(4 + TIME_MAX_DURATION_TICKS + 8), Ok(true));
        assert_eq!(timer.observe(3), Err(TimeError::ClockReversed));
        let mut timer = OneShotTimer::new();
        assert_eq!(
            timer.arm(4, TIME_MAX_DURATION_TICKS + 1),
            Err(TimeError::DurationTooLarge)
        );
    }

    #[test]
    fn debounce_leading_trailing_terminal_and_cancel_are_explicit() {
        let mut leading = DebounceState::new();
        assert_eq!(
            leading.admit(1, 3, DebounceMode::Leading),
            Ok(Admission::EmitNow)
        );
        assert_eq!(
            leading.admit(2, 3, DebounceMode::Leading),
            Ok(Admission::BlockUntil(5))
        );
        assert_eq!(leading.poll(5, DebounceMode::Leading), Ok(false));

        let mut trailing = DebounceState::new();
        assert_eq!(
            trailing.admit(1, 3, DebounceMode::Trailing),
            Ok(Admission::CoalescedUntil(4))
        );
        assert_eq!(
            trailing.admit(2, 3, DebounceMode::Trailing),
            Ok(Admission::CoalescedUntil(5))
        );
        assert_eq!(trailing.poll(4, DebounceMode::Trailing), Ok(false));
        assert!(trailing.finish(TerminalPendingPolicy::Flush));
        trailing.cancel();
        assert!(!trailing.finish(TerminalPendingPolicy::Flush));
    }

    #[test]
    fn throttle_distinguishes_lossless_block_from_explicit_coalescing() {
        let mut leading = ThrottleState::new();
        assert_eq!(
            leading.admit(10, 5, ThrottleMode::LeadingBlock),
            Ok(Admission::EmitNow)
        );
        assert_eq!(
            leading.admit(11, 5, ThrottleMode::LeadingBlock),
            Ok(Admission::BlockUntil(15))
        );
        assert_eq!(leading.poll(15, ThrottleMode::LeadingBlock), Ok(false));

        let mut trailing = ThrottleState::new();
        assert_eq!(
            trailing.admit(10, 5, ThrottleMode::TrailingCoalesce),
            Ok(Admission::RetainUntil(15))
        );
        assert_eq!(
            trailing.admit(11, 5, ThrottleMode::TrailingCoalesce),
            Ok(Admission::CoalescedUntil(15))
        );
        assert_eq!(trailing.poll(15, ThrottleMode::TrailingCoalesce), Ok(true));
    }
}
