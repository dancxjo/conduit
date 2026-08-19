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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SafetyHazardSet(u16);

impl SafetyHazardSet {
    pub const EMPTY: Self = Self(0);

    pub const fn contains(self, hazard: LocalHazard) -> bool {
        self.0 & hazard_bit(hazard) != 0
    }

    pub const fn insert(self, hazard: LocalHazard) -> Self {
        Self(self.0 | hazard_bit(hazard))
    }

    pub const fn remove(self, hazard: LocalHazard) -> Self {
        Self(self.0 & !hazard_bit(hazard))
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub(crate) const fn from_private_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub fn first(self) -> Option<LocalHazard> {
        HAZARD_PRIORITY
            .into_iter()
            .find(|hazard| self.contains(*hazard))
    }
}

const HAZARD_PRIORITY: [LocalHazard; 12] = [
    LocalHazard::EmergencyStop,
    LocalHazard::WheelDrop,
    LocalHazard::Cliff,
    LocalHazard::Contact,
    LocalHazard::Tilt,
    LocalHazard::Impact,
    LocalHazard::Charging,
    LocalHazard::ControlLost,
    LocalHazard::BodyLinkLost,
    LocalHazard::WatchdogUnhealthy,
    LocalHazard::SafetyGenerationRegressed,
    LocalHazard::SafetyClockInvalid,
];

const fn hazard_bit(hazard: LocalHazard) -> u16 {
    match hazard {
        LocalHazard::EmergencyStop => 1 << 0,
        LocalHazard::WheelDrop => 1 << 1,
        LocalHazard::Cliff => 1 << 2,
        LocalHazard::Contact => 1 << 3,
        LocalHazard::Tilt => 1 << 4,
        LocalHazard::Impact => 1 << 5,
        LocalHazard::Charging => 1 << 6,
        LocalHazard::ControlLost => 1 << 7,
        LocalHazard::BodyLinkLost => 1 << 8,
        LocalHazard::WatchdogUnhealthy => 1 << 9,
        LocalHazard::SafetyGenerationRegressed => 1 << 10,
        LocalHazard::SafetyClockInvalid => 1 << 11,
    }
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
    pub latch_generation: u32,
    pub latched_hazards: SafetyHazardSet,
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
        self.latched_hazards.first().or_else(|| {
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
        })
    }

    pub fn has_complete_independent_envelope(self) -> bool {
        self.independent_watchdog == IndependentWatchdogObservation::Healthy
            && self.latch_generation > 0
            && !matches!(self.emergency_stop, SafetyInputObservation::Unavailable)
            && !matches!(self.tilt, SafetyInputObservation::Unavailable)
            && !matches!(self.impact, SafetyInputObservation::Unavailable)
    }
}
