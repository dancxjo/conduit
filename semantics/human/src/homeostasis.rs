//! Bounded, factual homeostatic self-state derived by explicit policy.
//!
//! These values describe internal condition. They grant no authority and carry
//! no characterization such as hunger, fatigue, anxiety, or relief.

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{kind_id, SignId, TemporalInstant};

use crate::{
    BodySelfObservation, ExperienceCertainty, ExperienceSourceRef, MAXIMUM_BODY_SELF_STATE_BYTES,
};

pub const HOMEOSTATIC_STATE_KIND: &str = "experience/homeostatic-state@1";
pub const HOMEOSTASIS_POLICY_REVISION: &str = "conduit.homeostasis/thresholds@1";
pub const MAXIMUM_HOMEOSTATIC_SOURCES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAvailability {
    Present,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceObservation<T> {
    pub source_identity: String,
    pub availability: SourceAvailability,
    pub value: Option<T>,
    pub observation_sign_id: Option<SignId>,
    pub observed_at: Option<TemporalInstant>,
    pub freshness_limit_ticks: u64,
    pub uncertainty_permille: u16,
    pub calibration_profile_identity: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerCondition {
    pub reserve_permille: u16,
    pub charging: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThermalCondition {
    pub milli_celsius: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureCondition {
    pub pressure_permille: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyCondition {
    pub motion_available: bool,
    pub inhibited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityCondition {
    pub available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyState {
    Nominal,
    Low,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    Nominal,
    Constrained,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcePressureState {
    Nominal,
    High,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityState {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeostasisPolicy {
    pub revision: String,
    pub energy_low_permille: u16,
    pub energy_critical_permille: u16,
    pub thermal_constrained_milli_celsius: i32,
    pub thermal_critical_milli_celsius: i32,
    pub pressure_high_permille: u16,
    pub pressure_critical_permille: u16,
}

impl Default for HomeostasisPolicy {
    fn default() -> Self {
        Self {
            revision: HOMEOSTASIS_POLICY_REVISION.into(),
            energy_low_permille: 250,
            energy_critical_permille: 100,
            thermal_constrained_milli_celsius: 75_000,
            thermal_critical_milli_celsius: 90_000,
            pressure_high_permille: 800,
            pressure_critical_permille: 950,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeostaticInputs {
    pub power: SourceObservation<PowerCondition>,
    pub thermal: SourceObservation<ThermalCondition>,
    pub compute_pressure: SourceObservation<PressureCondition>,
    pub storage_pressure: SourceObservation<PressureCondition>,
    pub motion_safety: SourceObservation<SafetyCondition>,
    pub important_capability: SourceObservation<CapabilityCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeostaticState {
    pub policy_revision: String,
    pub energy: EnergyState,
    pub charging: Option<bool>,
    pub thermal: ThermalState,
    pub compute_pressure: ResourcePressureState,
    pub storage_pressure: ResourcePressureState,
    pub motion: AvailabilityState,
    pub important_capability: AvailabilityState,
    pub source_observations: Vec<HomeostaticSourceFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeostaticSourceFact {
    pub source_identity: String,
    pub availability: SourceAvailability,
    pub observation_sign_id: Option<SignId>,
    pub observed_at: Option<TemporalInstant>,
    pub freshness_limit_ticks: u64,
    pub uncertainty_permille: u16,
    pub calibration_profile_identity: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeostasisRefusal {
    InvalidPolicy,
    InvalidSource,
    InvalidObservation,
    FutureObservation,
    StaleObservation,
    SourceCapacity,
    EncodingCapacity,
}

pub fn reduce_homeostasis(
    reference_at: &TemporalInstant,
    policy: &HomeostasisPolicy,
    inputs: &HomeostaticInputs,
) -> Result<HomeostaticState, HomeostasisRefusal> {
    validate_policy(policy)?;
    reference_at
        .validate()
        .map_err(|_| HomeostasisRefusal::InvalidObservation)?;
    validate_observations(reference_at, inputs)?;

    let facts = vec![
        fact(&inputs.power),
        fact(&inputs.thermal),
        fact(&inputs.compute_pressure),
        fact(&inputs.storage_pressure),
        fact(&inputs.motion_safety),
        fact(&inputs.important_capability),
    ];
    if facts.len() > MAXIMUM_HOMEOSTATIC_SOURCES {
        return Err(HomeostasisRefusal::SourceCapacity);
    }
    Ok(HomeostaticState {
        policy_revision: policy.revision.clone(),
        energy: present(&inputs.power)
            .map(|value| {
                if value.reserve_permille <= policy.energy_critical_permille {
                    EnergyState::Critical
                } else if value.reserve_permille <= policy.energy_low_permille {
                    EnergyState::Low
                } else {
                    EnergyState::Nominal
                }
            })
            .unwrap_or(EnergyState::Unknown),
        charging: present(&inputs.power).map(|value| value.charging),
        thermal: present(&inputs.thermal)
            .map(|value| {
                if value.milli_celsius >= policy.thermal_critical_milli_celsius {
                    ThermalState::Critical
                } else if value.milli_celsius >= policy.thermal_constrained_milli_celsius {
                    ThermalState::Constrained
                } else {
                    ThermalState::Nominal
                }
            })
            .unwrap_or(ThermalState::Unknown),
        compute_pressure: pressure_state(&inputs.compute_pressure, policy),
        storage_pressure: pressure_state(&inputs.storage_pressure, policy),
        motion: present(&inputs.motion_safety)
            .map(|value| {
                if value.motion_available && !value.inhibited {
                    AvailabilityState::Available
                } else {
                    AvailabilityState::Unavailable
                }
            })
            .unwrap_or(AvailabilityState::Unknown),
        important_capability: present(&inputs.important_capability)
            .map(|value| {
                if value.available {
                    AvailabilityState::Available
                } else {
                    AvailabilityState::Unavailable
                }
            })
            .unwrap_or(AvailabilityState::Unknown),
        source_observations: facts,
    })
}

impl HomeostaticState {
    pub fn as_body_self_observation(
        &self,
        reduction_sign_id: SignId,
        observed_at: TemporalInstant,
    ) -> Result<BodySelfObservation, HomeostasisRefusal> {
        let canonical_state = self.canonical_bytes();
        if canonical_state.len() > MAXIMUM_BODY_SELF_STATE_BYTES {
            return Err(HomeostasisRefusal::EncodingCapacity);
        }
        let mut source_refs = Vec::with_capacity(self.source_observations.len() * 2);
        for fact in &self.source_observations {
            source_refs.push(ExperienceSourceRef::Source {
                source_id: fact.source_identity.clone(),
            });
            if let Some(sign) = &fact.observation_sign_id {
                source_refs.push(ExperienceSourceRef::Sign(sign.clone()));
            }
        }
        Ok(BodySelfObservation {
            state_kind: kind_id(HOMEOSTATIC_STATE_KIND),
            canonical_state,
            observation_sign_id: reduction_sign_id,
            observed_at,
            certainty: if self.source_observations.iter().any(|fact| {
                fact.availability != SourceAvailability::Present || fact.uncertainty_permille > 0
            }) {
                ExperienceCertainty::Uncertain
            } else {
                ExperienceCertainty::Certain
            },
            source_refs,
        })
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        // A compact deterministic semantic encoding; presentation prose is
        // intentionally absent. Exact source evidence remains in source_refs.
        alloc::format!(
            "policy={};energy={};charging={};thermal={};compute={};storage={};motion={};capability={}",
            self.policy_revision,
            energy_tag(self.energy),
            self.charging.map_or("unknown", |value| if value { "true" } else { "false" }),
            thermal_tag(self.thermal),
            pressure_tag(self.compute_pressure),
            pressure_tag(self.storage_pressure),
            availability_tag(self.motion),
            availability_tag(self.important_capability)
        )
        .into_bytes()
    }
}

const fn energy_tag(value: EnergyState) -> &'static str {
    match value {
        EnergyState::Nominal => "nominal",
        EnergyState::Low => "low",
        EnergyState::Critical => "critical",
        EnergyState::Unknown => "unknown",
    }
}

const fn thermal_tag(value: ThermalState) -> &'static str {
    match value {
        ThermalState::Nominal => "nominal",
        ThermalState::Constrained => "constrained",
        ThermalState::Critical => "critical",
        ThermalState::Unknown => "unknown",
    }
}

const fn pressure_tag(value: ResourcePressureState) -> &'static str {
    match value {
        ResourcePressureState::Nominal => "nominal",
        ResourcePressureState::High => "high",
        ResourcePressureState::Critical => "critical",
        ResourcePressureState::Unknown => "unknown",
    }
}

const fn availability_tag(value: AvailabilityState) -> &'static str {
    match value {
        AvailabilityState::Available => "available",
        AvailabilityState::Unavailable => "unavailable",
        AvailabilityState::Unknown => "unknown",
    }
}

fn validate_policy(policy: &HomeostasisPolicy) -> Result<(), HomeostasisRefusal> {
    if policy.revision.is_empty()
        || policy.revision.len() > 128
        || policy.energy_critical_permille > policy.energy_low_permille
        || policy.energy_low_permille > 1_000
        || policy.thermal_critical_milli_celsius <= policy.thermal_constrained_milli_celsius
        || policy.pressure_high_permille > policy.pressure_critical_permille
        || policy.pressure_critical_permille > 1_000
    {
        return Err(HomeostasisRefusal::InvalidPolicy);
    }
    Ok(())
}

fn validate_observations(
    reference: &TemporalInstant,
    inputs: &HomeostaticInputs,
) -> Result<(), HomeostasisRefusal> {
    validate(reference, &inputs.power, |v| v.reserve_permille <= 1_000)?;
    validate(reference, &inputs.thermal, |_| true)?;
    validate(reference, &inputs.compute_pressure, |v| {
        v.pressure_permille <= 1_000
    })?;
    validate(reference, &inputs.storage_pressure, |v| {
        v.pressure_permille <= 1_000
    })?;
    validate(reference, &inputs.motion_safety, |_| true)?;
    validate(reference, &inputs.important_capability, |_| true)
}

fn validate<T>(
    reference: &TemporalInstant,
    observation: &SourceObservation<T>,
    value_valid: impl FnOnce(&T) -> bool,
) -> Result<(), HomeostasisRefusal> {
    if observation.source_identity.is_empty()
        || observation.source_identity.len() > 128
        || observation.uncertainty_permille > 1_000
        || observation.freshness_limit_ticks == 0
        || observation
            .calibration_profile_identity
            .as_ref()
            .is_some_and(|identity| identity.is_empty() || identity.len() > 128)
    {
        return Err(HomeostasisRefusal::InvalidSource);
    }
    match observation.availability {
        SourceAvailability::Present => {
            let (Some(value), Some(sign), Some(at)) = (
                observation.value.as_ref(),
                observation.observation_sign_id.as_ref(),
                observation.observed_at.as_ref(),
            ) else {
                return Err(HomeostasisRefusal::InvalidObservation);
            };
            if sign.as_str().is_empty() || !value_valid(value) {
                return Err(HomeostasisRefusal::InvalidObservation);
            }
            let relation = at
                .relation_to(reference)
                .map_err(|_| HomeostasisRefusal::InvalidObservation)?;
            match relation {
                conduit_core::TemporalRelation::Future { .. } => {
                    return Err(HomeostasisRefusal::FutureObservation)
                }
                conduit_core::TemporalRelation::Past { minimum_ticks, .. }
                    if minimum_ticks > observation.freshness_limit_ticks =>
                {
                    return Err(HomeostasisRefusal::StaleObservation)
                }
                _ => {}
            }
        }
        SourceAvailability::Missing | SourceAvailability::Unavailable => {
            if observation.value.is_some()
                || observation.observation_sign_id.is_some()
                || observation.observed_at.is_some()
            {
                return Err(HomeostasisRefusal::InvalidObservation);
            }
        }
    }
    Ok(())
}

fn present<T>(observation: &SourceObservation<T>) -> Option<&T> {
    (observation.availability == SourceAvailability::Present)
        .then_some(observation.value.as_ref())
        .flatten()
}

fn pressure_state(
    observation: &SourceObservation<PressureCondition>,
    policy: &HomeostasisPolicy,
) -> ResourcePressureState {
    present(observation)
        .map(|value| {
            if value.pressure_permille >= policy.pressure_critical_permille {
                ResourcePressureState::Critical
            } else if value.pressure_permille >= policy.pressure_high_permille {
                ResourcePressureState::High
            } else {
                ResourcePressureState::Nominal
            }
        })
        .unwrap_or(ResourcePressureState::Unknown)
}

fn fact<T>(observation: &SourceObservation<T>) -> HomeostaticSourceFact {
    HomeostaticSourceFact {
        source_identity: observation.source_identity.clone(),
        availability: observation.availability,
        observation_sign_id: observation.observation_sign_id.clone(),
        observed_at: observation.observed_at.clone(),
        freshness_limit_ticks: observation.freshness_limit_ticks,
        uncertainty_permille: observation.uncertainty_permille,
        calibration_profile_identity: observation.calibration_profile_identity.clone(),
    }
}
