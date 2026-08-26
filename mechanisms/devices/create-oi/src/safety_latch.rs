//! Finite persistent latch below every physical Create actuator offer.

use crate::{
    IndependentWatchdogObservation, LocalHazard, SafetyHazardSet, SafetyInputObservation,
    SafetyObservation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafetyInputs {
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

impl SafetyInputs {
    pub fn active_hazards(self, now_tick: u64) -> SafetyHazardSet {
        let mut hazards = SafetyHazardSet::EMPTY;
        if self.observed_at_tick > now_tick {
            return hazards.insert(LocalHazard::SafetyClockInvalid);
        }
        if now_tick.saturating_sub(self.observed_at_tick) > u64::from(self.maximum_age_ticks) {
            hazards = hazards.insert(LocalHazard::BodyLinkLost);
        }
        for (active, hazard) in [
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
        ] {
            if active {
                hazards = hazards.insert(hazard);
            }
        }
        hazards
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyEnvelopeRefusal {
    InvalidObservationGeneration,
    ObservationGenerationRegressed,
    LatchGenerationExhausted,
    MissingExpectedLatchGeneration,
    LatchGenerationMismatch,
    ObservationGenerationMismatch,
    ObservationClockInvalid,
    ObservationStale,
    HazardNotLatched,
    HazardStillActive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyEnvelopeSign {
    Observed {
        observation_generation: u32,
        latch_generation: u32,
        newly_latched: SafetyHazardSet,
        active: SafetyHazardSet,
        latched: SafetyHazardSet,
    },
    EmergencyStopLatched {
        latch_generation: u32,
        latched: SafetyHazardSet,
    },
    Cleared {
        hazard: LocalHazard,
        observation_generation: u32,
        prior_latch_generation: u32,
        latch_generation: u32,
        remaining: SafetyHazardSet,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSafetyEnvelope {
    latest: Option<SafetyInputs>,
    latch_generation: u32,
    latched: SafetyHazardSet,
}

impl LocalSafetyEnvelope {
    pub const fn new() -> Self {
        Self {
            latest: None,
            latch_generation: 1,
            latched: SafetyHazardSet::EMPTY,
        }
    }

    pub fn observe(
        &mut self,
        inputs: SafetyInputs,
        now_tick: u64,
    ) -> Result<SafetyEnvelopeSign, SafetyEnvelopeRefusal> {
        if inputs.generation == 0 {
            self.latch_internal(LocalHazard::SafetyGenerationRegressed)?;
            return Err(SafetyEnvelopeRefusal::InvalidObservationGeneration);
        }
        if self
            .latest
            .is_some_and(|latest| inputs.generation <= latest.generation)
        {
            self.latch_internal(LocalHazard::SafetyGenerationRegressed)?;
            return Err(SafetyEnvelopeRefusal::ObservationGenerationRegressed);
        }
        let active = inputs.active_hazards(now_tick);
        let newly_latched =
            SafetyHazardSet::from_private_bits(active.bits() & !self.latched.bits());
        if !newly_latched.is_empty() {
            self.advance_latch_generation()?;
            self.latched = self.latched.union(newly_latched);
        }
        self.latest = Some(inputs);
        Ok(SafetyEnvelopeSign::Observed {
            observation_generation: inputs.generation,
            latch_generation: self.latch_generation,
            newly_latched,
            active,
            latched: self.latched,
        })
    }

    pub fn assert_emergency_stop(&mut self) -> Result<SafetyEnvelopeSign, SafetyEnvelopeRefusal> {
        if !self.latched.contains(LocalHazard::EmergencyStop) {
            self.advance_latch_generation()?;
            self.latched = self.latched.insert(LocalHazard::EmergencyStop);
        }
        Ok(SafetyEnvelopeSign::EmergencyStopLatched {
            latch_generation: self.latch_generation,
            latched: self.latched,
        })
    }

    pub fn clear(
        &mut self,
        hazard: LocalHazard,
        expected_latch_generation: u32,
        expected_observation_generation: u32,
        now_tick: u64,
    ) -> Result<SafetyEnvelopeSign, SafetyEnvelopeRefusal> {
        if expected_latch_generation == 0 {
            return Err(SafetyEnvelopeRefusal::MissingExpectedLatchGeneration);
        }
        if expected_latch_generation != self.latch_generation {
            return Err(SafetyEnvelopeRefusal::LatchGenerationMismatch);
        }
        let latest = self
            .latest
            .ok_or(SafetyEnvelopeRefusal::ObservationGenerationMismatch)?;
        if expected_observation_generation != latest.generation {
            return Err(SafetyEnvelopeRefusal::ObservationGenerationMismatch);
        }
        if latest.observed_at_tick > now_tick {
            return Err(SafetyEnvelopeRefusal::ObservationClockInvalid);
        }
        if now_tick.saturating_sub(latest.observed_at_tick) > u64::from(latest.maximum_age_ticks) {
            return Err(SafetyEnvelopeRefusal::ObservationStale);
        }
        if !self.latched.contains(hazard) {
            return Err(SafetyEnvelopeRefusal::HazardNotLatched);
        }
        if latest.active_hazards(now_tick).contains(hazard) {
            return Err(SafetyEnvelopeRefusal::HazardStillActive);
        }
        let prior_latch_generation = self.latch_generation;
        self.advance_latch_generation()?;
        self.latched = self.latched.remove(hazard);
        Ok(SafetyEnvelopeSign::Cleared {
            hazard,
            observation_generation: latest.generation,
            prior_latch_generation,
            latch_generation: self.latch_generation,
            remaining: self.latched,
        })
    }

    pub fn snapshot(self) -> Option<SafetyObservation> {
        let inputs = self.latest?;
        Some(SafetyObservation {
            generation: inputs.generation,
            latch_generation: self.latch_generation,
            latched_hazards: self.latched,
            observed_at_tick: inputs.observed_at_tick,
            maximum_age_ticks: inputs.maximum_age_ticks,
            emergency_stop: inputs.emergency_stop,
            wheel_drop: inputs.wheel_drop,
            cliff: inputs.cliff,
            contact: inputs.contact,
            tilt: inputs.tilt,
            impact: inputs.impact,
            charging: inputs.charging,
            control_alive: inputs.control_alive,
            body_link_alive: inputs.body_link_alive,
            independent_watchdog: inputs.independent_watchdog,
        })
    }

    fn advance_latch_generation(&mut self) -> Result<(), SafetyEnvelopeRefusal> {
        self.latch_generation = self
            .latch_generation
            .checked_add(1)
            .ok_or(SafetyEnvelopeRefusal::LatchGenerationExhausted)?;
        Ok(())
    }

    fn latch_internal(&mut self, hazard: LocalHazard) -> Result<(), SafetyEnvelopeRefusal> {
        if !self.latched.contains(hazard) {
            self.advance_latch_generation()?;
            self.latched = self.latched.insert(hazard);
        }
        Ok(())
    }
}

impl Default for LocalSafetyEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear(generation: u32, tick: u64) -> SafetyInputs {
        SafetyInputs {
            generation,
            observed_at_tick: tick,
            maximum_age_ticks: 10,
            emergency_stop: SafetyInputObservation::Clear,
            wheel_drop: false,
            cliff: false,
            contact: false,
            tilt: SafetyInputObservation::Clear,
            impact: SafetyInputObservation::Clear,
            charging: false,
            control_alive: true,
            body_link_alive: true,
            independent_watchdog: IndependentWatchdogObservation::Healthy,
        }
    }

    #[test]
    fn cleared_raw_input_remains_load_bearing_until_exact_conditional_clear() {
        let mut envelope = LocalSafetyEnvelope::new();
        let mut contact = clear(1, 100);
        contact.contact = true;
        let SafetyEnvelopeSign::Observed {
            latch_generation,
            newly_latched,
            ..
        } = envelope.observe(contact, 100).unwrap()
        else {
            panic!("expected observation Sign")
        };
        assert!(newly_latched.contains(LocalHazard::Contact));
        assert_eq!(latch_generation, 2);

        envelope.observe(clear(2, 101), 101).unwrap();
        let snapshot = envelope.snapshot().unwrap();
        assert_eq!(snapshot.first_hazard(101), Some(LocalHazard::Contact));
        assert_eq!(
            envelope.clear(LocalHazard::Contact, 1, 2, 101),
            Err(SafetyEnvelopeRefusal::LatchGenerationMismatch)
        );
        let cleared = envelope.clear(LocalHazard::Contact, 2, 2, 101).unwrap();
        assert!(matches!(
            cleared,
            SafetyEnvelopeSign::Cleared {
                remaining: SafetyHazardSet::EMPTY,
                latch_generation: 3,
                ..
            }
        ));
        assert_eq!(envelope.snapshot().unwrap().first_hazard(101), None);
    }

    #[test]
    fn simultaneous_hazards_are_independent_and_active_truth_cannot_be_cleared() {
        let mut envelope = LocalSafetyEnvelope::new();
        let mut hazards = clear(1, 100);
        hazards.wheel_drop = true;
        hazards.cliff = true;
        envelope.observe(hazards, 100).unwrap();
        assert_eq!(
            envelope.clear(LocalHazard::Cliff, 2, 1, 100),
            Err(SafetyEnvelopeRefusal::HazardStillActive)
        );
        let mut wheel_only = clear(2, 101);
        wheel_only.wheel_drop = true;
        envelope.observe(wheel_only, 101).unwrap();
        envelope.clear(LocalHazard::Cliff, 2, 2, 101).unwrap();
        let snapshot = envelope.snapshot().unwrap();
        assert!(snapshot.latched_hazards.contains(LocalHazard::WheelDrop));
        assert!(!snapshot.latched_hazards.contains(LocalHazard::Cliff));
    }

    #[test]
    fn emergency_stop_and_generation_regression_both_persist() {
        let mut envelope = LocalSafetyEnvelope::new();
        envelope.observe(clear(2, 100), 100).unwrap();
        envelope.assert_emergency_stop().unwrap();
        assert_eq!(
            envelope.observe(clear(1, 101), 101),
            Err(SafetyEnvelopeRefusal::ObservationGenerationRegressed)
        );
        let snapshot = envelope.snapshot().unwrap();
        assert!(snapshot
            .latched_hazards
            .contains(LocalHazard::EmergencyStop));
        assert!(snapshot
            .latched_hazards
            .contains(LocalHazard::SafetyGenerationRegressed));
    }

    #[test]
    fn stale_observation_never_authorizes_a_clear() {
        let mut envelope = LocalSafetyEnvelope::new();
        let mut contact = clear(1, 100);
        contact.contact = true;
        envelope.observe(contact, 100).unwrap();
        envelope.observe(clear(2, 101), 101).unwrap();
        assert_eq!(
            envelope.clear(LocalHazard::Contact, 2, 2, 112),
            Err(SafetyEnvelopeRefusal::ObservationStale)
        );
    }
}
