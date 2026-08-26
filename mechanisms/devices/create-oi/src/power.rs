//! Finite Create power-toggle pulse mechanism shared by embedded Hosts.

pub const MAXIMUM_POWER_TOGGLE_STAGE_TICKS: u32 = 5_000;

pub trait CreatePowerToggleProvider {
    type Error;

    fn is_available(&self) -> bool;
    fn set_output_low(&mut self) -> Result<(), Self::Error>;
    fn set_output_high(&mut self) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreatePowerPulseProfile {
    pub low_settle_ticks: u32,
    pub high_pulse_ticks: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatePowerPulseState {
    IdleLow,
    WaitingLowSettle { raise_at_tick: u64 },
    WaitingHighPulse { lower_at_tick: u64 },
    CompletedLow,
    FailedLow,
    FailedHighDispositionUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatePowerPulseFailure {
    InvalidProfile,
    InvalidState,
    ClockRegressed,
    DeadlineOverflow,
    ProviderUnavailable,
    DriveLowFailed,
    DriveHighFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatePowerPulseProgress {
    WaitingLowSettle { raise_at_tick: u64 },
    WaitingHighPulse { lower_at_tick: u64 },
    CompletedLow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreatePowerToggle {
    profile: CreatePowerPulseProfile,
    state: CreatePowerPulseState,
    last_tick: u64,
}

impl CreatePowerToggle {
    pub const fn new(profile: CreatePowerPulseProfile) -> Self {
        Self {
            profile,
            state: CreatePowerPulseState::IdleLow,
            last_tick: 0,
        }
    }

    pub const fn state(self) -> CreatePowerPulseState {
        self.state
    }

    pub fn start<P: CreatePowerToggleProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
    ) -> Result<CreatePowerPulseProgress, CreatePowerPulseFailure> {
        if self.state != CreatePowerPulseState::IdleLow {
            return Err(CreatePowerPulseFailure::InvalidState);
        }
        validate_profile(self.profile)?;
        require_provider(provider)?;
        let raise_at_tick = now_tick
            .checked_add(u64::from(self.profile.low_settle_ticks))
            .ok_or(CreatePowerPulseFailure::DeadlineOverflow)?;
        if provider.set_output_low().is_err() {
            self.state = CreatePowerPulseState::FailedLow;
            return Err(CreatePowerPulseFailure::DriveLowFailed);
        }
        self.last_tick = now_tick;
        self.state = CreatePowerPulseState::WaitingLowSettle { raise_at_tick };
        Ok(CreatePowerPulseProgress::WaitingLowSettle { raise_at_tick })
    }

    pub fn advance<P: CreatePowerToggleProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
    ) -> Result<CreatePowerPulseProgress, CreatePowerPulseFailure> {
        if now_tick < self.last_tick {
            self.state = match self.state {
                CreatePowerPulseState::WaitingHighPulse { .. } => {
                    CreatePowerPulseState::FailedHighDispositionUnknown
                }
                _ => CreatePowerPulseState::FailedLow,
            };
            return Err(CreatePowerPulseFailure::ClockRegressed);
        }
        self.last_tick = now_tick;
        match self.state {
            CreatePowerPulseState::WaitingLowSettle { raise_at_tick }
                if now_tick < raise_at_tick =>
            {
                Ok(CreatePowerPulseProgress::WaitingLowSettle { raise_at_tick })
            }
            CreatePowerPulseState::WaitingLowSettle { .. } => {
                if let Err(failure) = require_provider(provider) {
                    self.state = CreatePowerPulseState::FailedLow;
                    return Err(failure);
                }
                let Some(lower_at_tick) =
                    now_tick.checked_add(u64::from(self.profile.high_pulse_ticks))
                else {
                    self.state = CreatePowerPulseState::FailedLow;
                    return Err(CreatePowerPulseFailure::DeadlineOverflow);
                };
                if provider.set_output_high().is_err() {
                    self.state = CreatePowerPulseState::FailedLow;
                    return Err(CreatePowerPulseFailure::DriveHighFailed);
                }
                self.state = CreatePowerPulseState::WaitingHighPulse { lower_at_tick };
                Ok(CreatePowerPulseProgress::WaitingHighPulse { lower_at_tick })
            }
            CreatePowerPulseState::WaitingHighPulse { lower_at_tick }
                if now_tick < lower_at_tick =>
            {
                Ok(CreatePowerPulseProgress::WaitingHighPulse { lower_at_tick })
            }
            CreatePowerPulseState::WaitingHighPulse { .. } => {
                if !provider.is_available() {
                    self.state = CreatePowerPulseState::FailedHighDispositionUnknown;
                    return Err(CreatePowerPulseFailure::ProviderUnavailable);
                }
                if provider.set_output_low().is_err() {
                    self.state = CreatePowerPulseState::FailedHighDispositionUnknown;
                    return Err(CreatePowerPulseFailure::DriveLowFailed);
                }
                self.state = CreatePowerPulseState::CompletedLow;
                Ok(CreatePowerPulseProgress::CompletedLow)
            }
            _ => Err(CreatePowerPulseFailure::InvalidState),
        }
    }
}

fn validate_profile(profile: CreatePowerPulseProfile) -> Result<(), CreatePowerPulseFailure> {
    if profile.low_settle_ticks == 0
        || profile.high_pulse_ticks == 0
        || profile.low_settle_ticks > MAXIMUM_POWER_TOGGLE_STAGE_TICKS
        || profile.high_pulse_ticks > MAXIMUM_POWER_TOGGLE_STAGE_TICKS
    {
        return Err(CreatePowerPulseFailure::InvalidProfile);
    }
    Ok(())
}

fn require_provider<P: CreatePowerToggleProvider>(
    provider: &P,
) -> Result<(), CreatePowerPulseFailure> {
    provider
        .is_available()
        .then_some(())
        .ok_or(CreatePowerPulseFailure::ProviderUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[derive(Default)]
    struct Provider {
        available: bool,
        levels: Vec<bool>,
        fail_at: Option<usize>,
    }

    impl CreatePowerToggleProvider for Provider {
        type Error = ();

        fn is_available(&self) -> bool {
            self.available
        }

        fn set_output_low(&mut self) -> Result<(), Self::Error> {
            self.set(false)
        }

        fn set_output_high(&mut self) -> Result<(), Self::Error> {
            self.set(true)
        }
    }

    impl Provider {
        fn set(&mut self, level: bool) -> Result<(), ()> {
            if self.fail_at == Some(self.levels.len()) {
                return Err(());
            }
            self.levels.push(level);
            Ok(())
        }
    }

    fn provider() -> Provider {
        Provider {
            available: true,
            ..Provider::default()
        }
    }

    #[test]
    fn one_pulse_is_exactly_low_wait_high_wait_low() {
        let mut toggle = CreatePowerToggle::new(CreatePowerPulseProfile {
            low_settle_ticks: 5,
            high_pulse_ticks: 500,
        });
        let mut provider = provider();
        assert_eq!(
            toggle.start(&mut provider, 10),
            Ok(CreatePowerPulseProgress::WaitingLowSettle { raise_at_tick: 15 })
        );
        assert_eq!(provider.levels, [false]);
        toggle.advance(&mut provider, 14).unwrap();
        assert_eq!(provider.levels, [false]);
        assert_eq!(
            toggle.advance(&mut provider, 15),
            Ok(CreatePowerPulseProgress::WaitingHighPulse { lower_at_tick: 515 })
        );
        assert_eq!(provider.levels, [false, true]);
        toggle.advance(&mut provider, 514).unwrap();
        assert_eq!(provider.levels, [false, true]);
        assert_eq!(
            toggle.advance(&mut provider, 515),
            Ok(CreatePowerPulseProgress::CompletedLow)
        );
        assert_eq!(provider.levels, [false, true, false]);
        assert_eq!(
            toggle.advance(&mut provider, 516),
            Err(CreatePowerPulseFailure::InvalidState)
        );
        assert_eq!(provider.levels, [false, true, false]);
    }

    #[test]
    fn provider_and_clock_failures_never_emit_a_second_rising_edge() {
        let profile = CreatePowerPulseProfile {
            low_settle_ticks: 1,
            high_pulse_ticks: 2,
        };
        let mut absent = Provider::default();
        assert_eq!(
            CreatePowerToggle::new(profile).start(&mut absent, 1),
            Err(CreatePowerPulseFailure::ProviderUnavailable)
        );
        let mut provider = provider();
        let mut toggle = CreatePowerToggle::new(profile);
        toggle.start(&mut provider, 10).unwrap();
        assert_eq!(
            toggle.advance(&mut provider, 9),
            Err(CreatePowerPulseFailure::ClockRegressed)
        );
        assert_eq!(toggle.state(), CreatePowerPulseState::FailedLow);
        assert_eq!(
            toggle.advance(&mut provider, 11),
            Err(CreatePowerPulseFailure::InvalidState)
        );

        let mut toggle = CreatePowerToggle::new(profile);
        toggle.start(&mut provider, 20).unwrap();
        toggle.advance(&mut provider, 21).unwrap();
        provider.available = false;
        assert_eq!(
            toggle.advance(&mut provider, 23),
            Err(CreatePowerPulseFailure::ProviderUnavailable)
        );
        assert_eq!(
            toggle.state(),
            CreatePowerPulseState::FailedHighDispositionUnknown
        );
        provider.available = true;
        assert_eq!(
            toggle.advance(&mut provider, 24),
            Err(CreatePowerPulseFailure::InvalidState)
        );
        assert_eq!(provider.levels, [false, false, true]);
    }
}
