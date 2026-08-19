//! Exact local safety observations shared by every Create drive realization.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalHazard {
    EmergencyStop,
    WheelDrop,
    Cliff,
    Contact,
    Tilt,
    Impact,
    Charging,
    ControlLost,
    BodyLinkLost,
    WatchdogUnhealthy,
    SafetyGenerationRegressed,
    SafetyClockInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndependentWatchdogObservation {
    Absent,
    Healthy,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyInputObservation {
    Unavailable,
    Clear,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafetyObservation {
    pub generation: u32,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
    pub emergency_stop: SafetyInputObservation,
    pub wheel_drop: bool,
    pub cliff: bool,
    pub contact: bool,
    pub tilt: SafetyInputObservation,
    pub impact: SafetyInputObservation,
    pub charging: bool,
    pub control_alive: bool,
    pub body_link_alive: bool,
    pub independent_watchdog: IndependentWatchdogObservation,
}

impl SafetyObservation {
    pub fn first_hazard(self, now_tick: u64) -> Option<LocalHazard> {
        if self.observed_at_tick > now_tick {
            return Some(LocalHazard::SafetyClockInvalid);
        }
        if now_tick.saturating_sub(self.observed_at_tick) > u64::from(self.maximum_age_ticks) {
            return Some(LocalHazard::BodyLinkLost);
        }
        [
            (
                self.emergency_stop == SafetyInputObservation::Active,
                LocalHazard::EmergencyStop,
            ),
            (self.wheel_drop, LocalHazard::WheelDrop),
            (self.cliff, LocalHazard::Cliff),
            (self.contact, LocalHazard::Contact),
            (
                self.tilt == SafetyInputObservation::Active,
                LocalHazard::Tilt,
            ),
            (
                self.impact == SafetyInputObservation::Active,
                LocalHazard::Impact,
            ),
            (self.charging, LocalHazard::Charging),
            (!self.control_alive, LocalHazard::ControlLost),
            (!self.body_link_alive, LocalHazard::BodyLinkLost),
            (
                self.independent_watchdog == IndependentWatchdogObservation::Failed,
                LocalHazard::WatchdogUnhealthy,
            ),
        ]
        .into_iter()
        .find_map(|(active, hazard)| active.then_some(hazard))
    }

    pub fn has_complete_independent_envelope(self) -> bool {
        self.independent_watchdog == IndependentWatchdogObservation::Healthy
            && !matches!(self.emergency_stop, SafetyInputObservation::Unavailable)
            && !matches!(self.tilt, SafetyInputObservation::Unavailable)
            && !matches!(self.impact, SafetyInputObservation::Unavailable)
    }
}
