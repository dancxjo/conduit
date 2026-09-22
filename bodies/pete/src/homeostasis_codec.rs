//! Canonical typed encoding for reduced homeostatic state.

use conduit_core::{
    kind_id, InfoBool, SignId, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue, StructuredVariantCase, TemporalInstant,
};
use std::{vec, vec::Vec};

use crate::{
    AvailabilityState, EnergyState, HomeostasisRefusal, HomeostaticState, ResourcePressureState,
    ThermalState, HOMEOSTATIC_STATE_KIND,
};
use conduit_human::{
    BodySelfObservation, ExperienceCertainty, ExperienceSourceRef, SourceAvailability,
    MAXIMUM_BODY_SELF_STATE_BYTES,
};

impl HomeostaticState {
    pub fn as_body_self_observation(
        &self,
        reduction_sign_id: SignId,
        observed_at: TemporalInstant,
    ) -> Result<BodySelfObservation, HomeostasisRefusal> {
        let canonical_state = self
            .canonical_value()?
            .canonical_bytes()
            .map_err(|_| HomeostasisRefusal::EncodingCapacity)?;
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

    pub fn canonical_value(&self) -> Result<StructuredInfoValue, HomeostasisRefusal> {
        StructuredInfoValue::record(
            homeostatic_state_type(),
            vec![
                value_field("charging", optional_bool(self.charging)?),
                value_field("compute_pressure", pressure_value(self.compute_pressure)?),
                value_field("energy", energy_value(self.energy)?),
                value_field(
                    "important_capability",
                    availability_value(self.important_capability)?,
                ),
                value_field("motion", availability_value(self.motion)?),
                value_field(
                    "policy_revision",
                    leaf_value("value/text", self.policy_revision.as_bytes().to_vec())?,
                ),
                value_field("storage_pressure", pressure_value(self.storage_pressure)?),
                value_field("thermal", thermal_value(self.thermal)?),
            ],
        )
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
    }
}

pub fn homeostatic_state_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(HOMEOSTATIC_STATE_KIND),
        vec![
            field("charging", optional_bool_type()),
            field("compute_pressure", pressure_type()),
            field("energy", energy_type()),
            field("important_capability", availability_type()),
            field("motion", availability_type()),
            field("policy_revision", leaf("value/text")),
            field("storage_pressure", pressure_type()),
            field("thermal", thermal_type()),
        ],
    )
    .expect("reviewed homeostatic state type")
}

fn leaf(identity: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(identity)).unwrap()
}
fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).unwrap()
}
fn value_field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).unwrap()
}
fn leaf_value(identity: &str, bytes: Vec<u8>) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::leaf(leaf(identity), bytes)
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn unit_value() -> Result<StructuredInfoValue, HomeostasisRefusal> {
    leaf_value("value/unit", Vec::new())
}

fn variant_type(identity: &str, tags: &[&str]) -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id(identity),
        tags.iter()
            .map(|tag| StructuredVariantCase::new(*tag, leaf("value/unit")).unwrap())
            .collect(),
    )
    .unwrap()
}
fn variant_value(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::variant(value_type, tag, unit_value()?)
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn energy_type() -> StructuredInfoType {
    variant_type(
        "experience/energy-state@1",
        &["nominal", "low", "critical", "unknown"],
    )
}
fn thermal_type() -> StructuredInfoType {
    variant_type(
        "experience/thermal-state@1",
        &["nominal", "constrained", "critical", "unknown"],
    )
}
fn pressure_type() -> StructuredInfoType {
    variant_type(
        "experience/resource-pressure-state@1",
        &["nominal", "high", "critical", "unknown"],
    )
}
fn availability_type() -> StructuredInfoType {
    variant_type(
        "experience/availability-state@1",
        &["available", "unavailable", "unknown"],
    )
}

fn energy_value(value: EnergyState) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    variant_value(
        energy_type(),
        match value {
            EnergyState::Nominal => "nominal",
            EnergyState::Low => "low",
            EnergyState::Critical => "critical",
            EnergyState::Unknown => "unknown",
        },
    )
}
fn thermal_value(value: ThermalState) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    variant_value(
        thermal_type(),
        match value {
            ThermalState::Nominal => "nominal",
            ThermalState::Constrained => "constrained",
            ThermalState::Critical => "critical",
            ThermalState::Unknown => "unknown",
        },
    )
}
fn pressure_value(value: ResourcePressureState) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    variant_value(
        pressure_type(),
        match value {
            ResourcePressureState::Nominal => "nominal",
            ResourcePressureState::High => "high",
            ResourcePressureState::Critical => "critical",
            ResourcePressureState::Unknown => "unknown",
        },
    )
}
fn availability_value(value: AvailabilityState) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    variant_value(
        availability_type(),
        match value {
            AvailabilityState::Available => "available",
            AvailabilityState::Unavailable => "unavailable",
            AvailabilityState::Unknown => "unknown",
        },
    )
}

fn optional_bool_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("experience/optional-bool@1"),
        vec![
            StructuredVariantCase::new("known", leaf("value/bool")).unwrap(),
            StructuredVariantCase::new("unknown", leaf("value/unit")).unwrap(),
        ],
    )
    .unwrap()
}
fn optional_bool(value: Option<bool>) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    let (tag, payload) = match value {
        Some(value) => (
            "known",
            leaf_value("value/bool", InfoBool::new(value).encode().to_vec())?,
        ),
        None => ("unknown", unit_value()?),
    };
    StructuredInfoValue::variant(optional_bool_type(), tag, payload)
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
