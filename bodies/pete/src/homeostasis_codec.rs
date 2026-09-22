//! Canonical typed encoding for reduced homeostatic state.

use conduit_core::{
    encode_count, kind_id, InfoBool, Scalar, SignId, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue, StructuredVariantCase, TemporalInstant, TemporalScale,
};
use std::{vec, vec::Vec};

use crate::{
    AvailabilityState, CapabilityCondition, EnergyState, HomeostasisRefusal, HomeostaticState,
    MotionState, PowerCondition, PressureCondition, ResourcePressureState, SafetyCondition,
    ThermalCondition, ThermalState, HOMEOSTATIC_STATE_KIND,
};
use conduit_human::{
    BodySelfObservation, ExperienceCertainty, ExperienceSourceRef, SourceAvailability,
    MAXIMUM_BODY_SELF_STATE_BYTES,
};

pub fn decode_power_observation(
    bytes: &[u8],
) -> Result<conduit_human::SourceObservation<PowerCondition>, HomeostasisRefusal> {
    decode_observation(bytes, |value| {
        Ok(PowerCondition {
            charging: decode_bool(record_field(value, "charging")?)?,
            reserve_permille: decode_count_value(record_field(value, "reserve_permille")?)?
                .try_into()
                .map_err(|_| HomeostasisRefusal::InvalidObservation)?,
        })
    })
}
pub fn decode_thermal_observation(
    bytes: &[u8],
) -> Result<conduit_human::SourceObservation<ThermalCondition>, HomeostasisRefusal> {
    decode_observation(bytes, |value| {
        Ok(ThermalCondition {
            milli_celsius: Scalar::decode(leaf_bytes(record_field(value, "milli_celsius")?)?)
                .map_err(|_| HomeostasisRefusal::InvalidObservation)?
                .raw_microunits()
                .try_into()
                .map_err(|_| HomeostasisRefusal::InvalidObservation)?,
        })
    })
}
pub fn decode_pressure_observation(
    bytes: &[u8],
) -> Result<conduit_human::SourceObservation<PressureCondition>, HomeostasisRefusal> {
    decode_observation(bytes, |value| {
        Ok(PressureCondition {
            pressure_permille: decode_count_value(record_field(value, "pressure_permille")?)?
                .try_into()
                .map_err(|_| HomeostasisRefusal::InvalidObservation)?,
        })
    })
}
pub fn decode_safety_observation(
    bytes: &[u8],
) -> Result<conduit_human::SourceObservation<SafetyCondition>, HomeostasisRefusal> {
    decode_observation(bytes, |value| {
        Ok(SafetyCondition {
            inhibited: decode_bool(record_field(value, "inhibited")?)?,
            motion_available: decode_bool(record_field(value, "motion_available")?)?,
        })
    })
}
pub fn decode_capability_observation(
    bytes: &[u8],
) -> Result<conduit_human::SourceObservation<CapabilityCondition>, HomeostasisRefusal> {
    decode_observation(bytes, |value| {
        Ok(CapabilityCondition {
            available: decode_bool(record_field(value, "available")?)?,
        })
    })
}
pub fn decode_reduction_instant(bytes: &[u8]) -> Result<TemporalInstant, HomeostasisRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(bytes)
        .map_err(|_| HomeostasisRefusal::InvalidObservation)?;
    decode_instant(&value)
}

pub fn encode_power_observation(
    value: &conduit_human::SourceObservation<PowerCondition>,
) -> Result<Vec<u8>, HomeostasisRefusal> {
    power_observation_value(value)?
        .canonical_bytes()
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
pub fn encode_thermal_observation(
    value: &conduit_human::SourceObservation<ThermalCondition>,
) -> Result<Vec<u8>, HomeostasisRefusal> {
    thermal_observation_value(value)?
        .canonical_bytes()
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
pub fn encode_pressure_observation(
    value: &conduit_human::SourceObservation<PressureCondition>,
    storage: bool,
) -> Result<Vec<u8>, HomeostasisRefusal> {
    pressure_observation_value(
        value,
        if storage {
            "PeteStoragePressureObservation"
        } else {
            "PeteComputePressureObservation"
        },
    )?
    .canonical_bytes()
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
pub fn encode_safety_observation(
    value: &conduit_human::SourceObservation<SafetyCondition>,
) -> Result<Vec<u8>, HomeostasisRefusal> {
    safety_observation_value(value)?
        .canonical_bytes()
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
pub fn encode_capability_observation(
    value: &conduit_human::SourceObservation<CapabilityCondition>,
) -> Result<Vec<u8>, HomeostasisRefusal> {
    capability_observation_value(value)?
        .canonical_bytes()
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
pub fn encode_reduction_instant(value: &TemporalInstant) -> Result<Vec<u8>, HomeostasisRefusal> {
    instant_value(value)?
        .canonical_bytes()
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}

fn decode_observation<T>(
    bytes: &[u8],
    payload: impl FnOnce(&StructuredInfoValue) -> Result<T, HomeostasisRefusal>,
) -> Result<conduit_human::SourceObservation<T>, HomeostasisRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(bytes)
        .map_err(|_| HomeostasisRefusal::InvalidObservation)?;
    let availability = record_field(&value, "availability")?;
    let (availability, item, sign, at) = match availability.shape() {
        conduit_core::StructuredInfoValueShape::Variant { tag: "missing", .. } => {
            (SourceAvailability::Missing, None, None, None)
        }
        conduit_core::StructuredInfoValueShape::Variant {
            tag: "present",
            payload: evidence,
        } => (
            SourceAvailability::Present,
            Some(payload(record_field(evidence, "value")?)?),
            Some(SignId::from(decode_text(record_field(
                evidence,
                "observation_sign_identity",
            )?)?)),
            Some(decode_instant(record_field(evidence, "observed_at")?)?),
        ),
        conduit_core::StructuredInfoValueShape::Variant {
            tag: "unavailable",
            payload: evidence,
        } => (
            SourceAvailability::Unavailable,
            None,
            Some(SignId::from(decode_text(record_field(
                evidence,
                "observation_sign_identity",
            )?)?)),
            Some(decode_instant(record_field(evidence, "observed_at")?)?),
        ),
        _ => return Err(HomeostasisRefusal::InvalidObservation),
    };
    let calibration_profile_identity = match record_field(&value, "calibration_profile")?.shape() {
        conduit_core::StructuredInfoValueShape::Variant {
            tag: "known",
            payload,
        } => Some(decode_text(payload)?),
        conduit_core::StructuredInfoValueShape::Variant { tag: "none", .. } => None,
        _ => return Err(HomeostasisRefusal::InvalidObservation),
    };
    Ok(conduit_human::SourceObservation {
        source_identity: decode_text(record_field(&value, "source_identity")?)?,
        subject_identity: decode_text(record_field(&value, "subject_identity")?)?,
        availability,
        value: item,
        observation_sign_id: sign,
        observed_at: at,
        freshness_limit_ticks: decode_count_value(record_field(&value, "freshness_limit_ticks")?)?,
        uncertainty_permille: decode_count_value(record_field(&value, "uncertainty_permille")?)?
            .try_into()
            .map_err(|_| HomeostasisRefusal::InvalidObservation)?,
        calibration_profile_identity,
    })
}
fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, HomeostasisRefusal> {
    match value.shape() {
        conduit_core::StructuredInfoValueShape::Record(fields) => fields
            .iter()
            .find(|field| field.name() == name)
            .map(|field| field.value())
            .ok_or(HomeostasisRefusal::InvalidObservation),
        _ => Err(HomeostasisRefusal::InvalidObservation),
    }
}
fn leaf_bytes(value: &StructuredInfoValue) -> Result<&[u8], HomeostasisRefusal> {
    match value.shape() {
        conduit_core::StructuredInfoValueShape::Leaf(bytes) => Ok(bytes),
        _ => Err(HomeostasisRefusal::InvalidObservation),
    }
}
fn decode_text(value: &StructuredInfoValue) -> Result<String, HomeostasisRefusal> {
    String::from_utf8(leaf_bytes(value)?.to_vec())
        .map_err(|_| HomeostasisRefusal::InvalidObservation)
}
fn decode_count_value(value: &StructuredInfoValue) -> Result<u64, HomeostasisRefusal> {
    conduit_core::decode_count(leaf_bytes(value)?)
        .map_err(|_| HomeostasisRefusal::InvalidObservation)
}
fn decode_bool(value: &StructuredInfoValue) -> Result<bool, HomeostasisRefusal> {
    Ok(InfoBool::decode(leaf_bytes(value)?)
        .map_err(|_| HomeostasisRefusal::InvalidObservation)?
        .get())
}
fn decode_instant(value: &StructuredInfoValue) -> Result<TemporalInstant, HomeostasisRefusal> {
    let scale = match record_field(value, "scale")?.shape() {
        conduit_core::StructuredInfoValueShape::Variant { tag: "seconds", .. } => {
            TemporalScale::Seconds
        }
        conduit_core::StructuredInfoValueShape::Variant {
            tag: "milliseconds",
            ..
        } => TemporalScale::Milliseconds,
        conduit_core::StructuredInfoValueShape::Variant {
            tag: "microseconds",
            ..
        } => TemporalScale::Microseconds,
        conduit_core::StructuredInfoValueShape::Variant {
            tag: "nanoseconds", ..
        } => TemporalScale::Nanoseconds,
        _ => return Err(HomeostasisRefusal::InvalidObservation),
    };
    Ok(TemporalInstant {
        clock_basis: decode_text(record_field(value, "clock_basis")?)?,
        resolution_ticks: decode_count_value(record_field(value, "resolution_ticks")?)?,
        scale,
        ticks: decode_count_value(record_field(value, "ticks")?)?,
        uncertainty_ticks: decode_count_value(record_field(value, "uncertainty_ticks")?)?,
    })
}

impl HomeostaticState {
    pub fn as_body_self_observation(
        &self,
        reduction_sign_id: SignId,
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
            observed_at: self.reduced_at.clone(),
            certainty: ExperienceCertainty::Certain,
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
                value_field(
                    "important_capability_identity",
                    text_value(&self.important_capability_identity)?,
                ),
                value_field("motion", motion_value(self.motion)?),
                value_field("policy_identity", text_value(&self.policy_identity)?),
                value_field("policy_revision", text_value(&self.policy_revision)?),
                value_field("reduced_at", instant_value(&self.reduced_at)?),
                value_field(
                    "source_observations",
                    source_evidence_values(&self.source_inputs)?,
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
            field("important_capability_identity", leaf("value/text")),
            field("motion", motion_type()),
            field("policy_identity", leaf("value/text")),
            field("policy_revision", leaf("value/text")),
            field(
                "reduced_at",
                crate::homeostasis_catalog::temporal_instant_type(),
            ),
            field("source_observations", source_evidence_collection_type()),
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
fn motion_type() -> StructuredInfoType {
    variant_type(
        "pete/motion-state@1",
        &["available", "inhibited", "unavailable", "unknown"],
    )
}
fn observation_type(name: &str) -> StructuredInfoType {
    crate::homeostasis_registered_types()
        .into_iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, value_type)| value_type)
        .expect("registered Pete observation type")
}
fn source_value_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("pete/homeostatic-source-value@1"),
        vec![
            StructuredVariantCase::new("capability", capability_payload_type()).unwrap(),
            StructuredVariantCase::new("power", power_payload_type()).unwrap(),
            StructuredVariantCase::new("pressure", pressure_payload_type()).unwrap(),
            StructuredVariantCase::new("safety", safety_payload_type()).unwrap(),
            StructuredVariantCase::new("thermal", thermal_payload_type()).unwrap(),
        ],
    )
    .unwrap()
}
fn source_role_type() -> StructuredInfoType {
    variant_type(
        "pete/homeostatic-source-role@1",
        &[
            "compute_pressure",
            "important_capability",
            "motion_safety",
            "power",
            "storage_pressure",
            "thermal",
        ],
    )
}
fn source_evidence_type() -> StructuredInfoType {
    let present = StructuredInfoType::record(
        kind_id("pete/source-present@1"),
        vec![
            field(
                "observed_at",
                crate::homeostasis_catalog::temporal_instant_type(),
            ),
            field("observation_sign_identity", leaf("value/text")),
            field("value", source_value_type()),
        ],
    )
    .unwrap();
    let unavailable = StructuredInfoType::record(
        kind_id("pete/source-unavailable@1"),
        vec![
            field(
                "observed_at",
                crate::homeostasis_catalog::temporal_instant_type(),
            ),
            field("observation_sign_identity", leaf("value/text")),
        ],
    )
    .unwrap();
    let availability = StructuredInfoType::variant(
        kind_id("pete/source-availability@1"),
        vec![
            StructuredVariantCase::new("missing", leaf("value/unit")).unwrap(),
            StructuredVariantCase::new("present", present).unwrap(),
            StructuredVariantCase::new("unavailable", unavailable).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoType::record(
        kind_id("pete/homeostatic-source-evidence@1"),
        vec![
            field("availability", availability),
            field("calibration_profile", optional_calibration_type()),
            field("freshness_limit_ticks", leaf("value/count")),
            field("role", source_role_type()),
            field("source_identity", leaf("value/text")),
            field("subject_identity", leaf("value/text")),
            field("uncertainty_permille", leaf("value/count")),
        ],
    )
    .unwrap()
}
fn source_evidence_collection_type() -> StructuredInfoType {
    StructuredInfoType::collection(source_evidence_type(), Some(6)).unwrap()
}

fn source_evidence_values(
    inputs: &crate::HomeostaticInputs,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    let values = vec![
        source_evidence_value(
            &inputs.compute_pressure,
            "compute_pressure",
            "pressure",
            pressure_payload_value,
        ),
        source_evidence_value(
            &inputs.important_capability,
            "important_capability",
            "capability",
            capability_payload_value,
        ),
        source_evidence_value(
            &inputs.motion_safety,
            "motion_safety",
            "safety",
            safety_payload_value,
        ),
        source_evidence_value(&inputs.power, "power", "power", power_payload_value),
        source_evidence_value(
            &inputs.storage_pressure,
            "storage_pressure",
            "pressure",
            pressure_payload_value,
        ),
        source_evidence_value(&inputs.thermal, "thermal", "thermal", thermal_payload_value),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::collection(source_evidence_collection_type(), values)
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn source_evidence_value<T>(
    observation: &conduit_human::SourceObservation<T>,
    role: &str,
    value_tag: &str,
    payload: impl FnOnce(&T) -> Result<StructuredInfoValue, HomeostasisRefusal>,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    let evidence_type = source_evidence_type();
    let availability_type = match evidence_type.shape() {
        conduit_core::StructuredInfoTypeShape::Record { fields, .. } => fields
            .iter()
            .find(|field| field.name() == "availability")
            .unwrap()
            .value_type()
            .clone(),
        _ => unreachable!(),
    };
    let (tag, availability_payload) = match observation.availability {
        SourceAvailability::Missing => ("missing", unit_value()?),
        SourceAvailability::Present => {
            let present_type = record_type_case(&availability_type, "present")?;
            let raw = observation
                .value
                .as_ref()
                .ok_or(HomeostasisRefusal::InvalidObservation)?;
            (
                "present",
                StructuredInfoValue::record(
                    present_type,
                    vec![
                        value_field(
                            "observed_at",
                            instant_value(
                                observation
                                    .observed_at
                                    .as_ref()
                                    .ok_or(HomeostasisRefusal::InvalidObservation)?,
                            )?,
                        ),
                        value_field(
                            "observation_sign_identity",
                            text_value(
                                observation
                                    .observation_sign_id
                                    .as_ref()
                                    .ok_or(HomeostasisRefusal::InvalidObservation)?
                                    .as_str(),
                            )?,
                        ),
                        value_field(
                            "value",
                            StructuredInfoValue::variant(
                                source_value_type(),
                                value_tag,
                                payload(raw)?,
                            )
                            .map_err(|_| HomeostasisRefusal::EncodingCapacity)?,
                        ),
                    ],
                )
                .map_err(|_| HomeostasisRefusal::EncodingCapacity)?,
            )
        }
        SourceAvailability::Unavailable => {
            let unavailable_type = record_type_case(&availability_type, "unavailable")?;
            (
                "unavailable",
                StructuredInfoValue::record(
                    unavailable_type,
                    vec![
                        value_field(
                            "observed_at",
                            instant_value(
                                observation
                                    .observed_at
                                    .as_ref()
                                    .ok_or(HomeostasisRefusal::InvalidObservation)?,
                            )?,
                        ),
                        value_field(
                            "observation_sign_identity",
                            text_value(
                                observation
                                    .observation_sign_id
                                    .as_ref()
                                    .ok_or(HomeostasisRefusal::InvalidObservation)?
                                    .as_str(),
                            )?,
                        ),
                    ],
                )
                .map_err(|_| HomeostasisRefusal::EncodingCapacity)?,
            )
        }
    };
    let calibration = match &observation.calibration_profile_identity {
        Some(identity) => StructuredInfoValue::variant(
            optional_calibration_type(),
            "known",
            text_value(identity)?,
        ),
        None => StructuredInfoValue::variant(optional_calibration_type(), "none", unit_value()?),
    }
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)?;
    StructuredInfoValue::record(
        evidence_type,
        vec![
            value_field(
                "availability",
                StructuredInfoValue::variant(availability_type, tag, availability_payload)
                    .map_err(|_| HomeostasisRefusal::EncodingCapacity)?,
            ),
            value_field("calibration_profile", calibration),
            value_field(
                "freshness_limit_ticks",
                count_value(observation.freshness_limit_ticks)?,
            ),
            value_field("role", variant_value(source_role_type(), role)?),
            value_field("source_identity", text_value(&observation.source_identity)?),
            value_field(
                "subject_identity",
                text_value(&observation.subject_identity)?,
            ),
            value_field(
                "uncertainty_permille",
                count_value(u64::from(observation.uncertainty_permille))?,
            ),
        ],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
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
fn motion_value(value: MotionState) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    variant_value(
        motion_type(),
        match value {
            MotionState::Available => "available",
            MotionState::Inhibited => "inhibited",
            MotionState::Unavailable => "unavailable",
            MotionState::Unknown => "unknown",
        },
    )
}

fn text_value(value: &str) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    leaf_value("value/text", value.as_bytes().to_vec())
}
fn count_value(value: u64) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    leaf_value("value/count", encode_count(value).to_vec())
}
fn scalar_value(value: i64) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    leaf_value(
        "value/scalar",
        Scalar::from_raw_microunits(value).encode().to_vec(),
    )
}
fn instant_value(value: &TemporalInstant) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::record(
        crate::homeostasis_catalog::temporal_instant_type(),
        vec![
            value_field("clock_basis", text_value(&value.clock_basis)?),
            value_field("resolution_ticks", count_value(value.resolution_ticks)?),
            value_field(
                "scale",
                variant_value(
                    StructuredInfoType::variant(
                        kind_id("time/scale@1"),
                        ["seconds", "milliseconds", "microseconds", "nanoseconds"]
                            .into_iter()
                            .map(|tag| StructuredVariantCase::new(tag, leaf("value/unit")).unwrap())
                            .collect(),
                    )
                    .unwrap(),
                    match value.scale {
                        TemporalScale::Seconds => "seconds",
                        TemporalScale::Milliseconds => "milliseconds",
                        TemporalScale::Microseconds => "microseconds",
                        TemporalScale::Nanoseconds => "nanoseconds",
                    },
                )?,
            ),
            value_field("ticks", count_value(value.ticks)?),
            value_field("uncertainty_ticks", count_value(value.uncertainty_ticks)?),
        ],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}

fn observation_value<T>(
    observation: &conduit_human::SourceObservation<T>,
    type_name: &str,
    payload: impl FnOnce(&T) -> Result<StructuredInfoValue, HomeostasisRefusal>,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    let value_type = observation_type(type_name);
    let availability_type = match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Record { fields, .. } => fields
            .iter()
            .find(|field| field.name() == "availability")
            .unwrap()
            .value_type()
            .clone(),
        _ => unreachable!(),
    };
    let (tag, availability_payload) = match observation.availability {
        SourceAvailability::Missing => ("missing", unit_value()?),
        SourceAvailability::Present => {
            let value = observation
                .value
                .as_ref()
                .ok_or(HomeostasisRefusal::InvalidObservation)?;
            let at = observation
                .observed_at
                .as_ref()
                .ok_or(HomeostasisRefusal::InvalidObservation)?;
            let sign = observation
                .observation_sign_id
                .as_ref()
                .ok_or(HomeostasisRefusal::InvalidObservation)?;
            let present_type = record_type_case(&availability_type, "present")?;
            (
                "present",
                StructuredInfoValue::record(
                    present_type,
                    vec![
                        value_field("observed_at", instant_value(at)?),
                        value_field("observation_sign_identity", text_value(sign.as_str())?),
                        value_field("value", payload(value)?),
                    ],
                )
                .map_err(|_| HomeostasisRefusal::EncodingCapacity)?,
            )
        }
        SourceAvailability::Unavailable => {
            let at = observation
                .observed_at
                .as_ref()
                .ok_or(HomeostasisRefusal::InvalidObservation)?;
            let sign = observation
                .observation_sign_id
                .as_ref()
                .ok_or(HomeostasisRefusal::InvalidObservation)?;
            let unavailable_type = record_type_case(&availability_type, "unavailable")?;
            (
                "unavailable",
                StructuredInfoValue::record(
                    unavailable_type,
                    vec![
                        value_field("observed_at", instant_value(at)?),
                        value_field("observation_sign_identity", text_value(sign.as_str())?),
                    ],
                )
                .map_err(|_| HomeostasisRefusal::EncodingCapacity)?,
            )
        }
    };
    let calibration = match &observation.calibration_profile_identity {
        Some(identity) => StructuredInfoValue::variant(
            optional_calibration_type(),
            "known",
            text_value(identity)?,
        ),
        None => StructuredInfoValue::variant(optional_calibration_type(), "none", unit_value()?),
    }
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)?;
    StructuredInfoValue::record(
        value_type,
        vec![
            value_field(
                "availability",
                StructuredInfoValue::variant(availability_type, tag, availability_payload)
                    .map_err(|_| HomeostasisRefusal::EncodingCapacity)?,
            ),
            value_field("calibration_profile", calibration),
            value_field(
                "freshness_limit_ticks",
                count_value(observation.freshness_limit_ticks)?,
            ),
            value_field("source_identity", text_value(&observation.source_identity)?),
            value_field(
                "subject_identity",
                text_value(&observation.subject_identity)?,
            ),
            value_field(
                "uncertainty_permille",
                count_value(u64::from(observation.uncertainty_permille))?,
            ),
        ],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn record_type_case(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, HomeostasisRefusal> {
    match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Variant { cases, .. } => cases
            .iter()
            .find(|case| case.tag() == tag)
            .map(|case| case.payload_type().clone())
            .ok_or(HomeostasisRefusal::EncodingCapacity),
        _ => Err(HomeostasisRefusal::EncodingCapacity),
    }
}
fn optional_calibration_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("experience/optional-calibration-profile@1"),
        vec![
            StructuredVariantCase::new("known", leaf("value/text")).unwrap(),
            StructuredVariantCase::new("none", leaf("value/unit")).unwrap(),
        ],
    )
    .unwrap()
}
fn power_payload_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("observation/energy-reserve-and-charging@1"),
        vec![
            field("charging", leaf("value/bool")),
            field("reserve_permille", leaf("value/count")),
        ],
    )
    .unwrap()
}
fn thermal_payload_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("observation/temperature-milli-celsius@1"),
        vec![field("milli_celsius", leaf("value/scalar"))],
    )
    .unwrap()
}
fn pressure_payload_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("observation/pressure-permille@1"),
        vec![field("pressure_permille", leaf("value/count"))],
    )
    .unwrap()
}
fn safety_payload_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("pete/motion-safety-condition@1"),
        vec![
            field("inhibited", leaf("value/bool")),
            field("motion_available", leaf("value/bool")),
        ],
    )
    .unwrap()
}
fn capability_payload_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("observation/capability-availability@1"),
        vec![field("available", leaf("value/bool"))],
    )
    .unwrap()
}
fn power_payload_value(v: &PowerCondition) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::record(
        power_payload_type(),
        vec![
            value_field(
                "charging",
                leaf_value("value/bool", InfoBool::new(v.charging).encode().to_vec())?,
            ),
            value_field(
                "reserve_permille",
                count_value(u64::from(v.reserve_permille))?,
            ),
        ],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn thermal_payload_value(v: &ThermalCondition) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::record(
        thermal_payload_type(),
        vec![value_field(
            "milli_celsius",
            scalar_value(i64::from(v.milli_celsius))?,
        )],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn pressure_payload_value(
    v: &PressureCondition,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::record(
        pressure_payload_type(),
        vec![value_field(
            "pressure_permille",
            count_value(u64::from(v.pressure_permille))?,
        )],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn safety_payload_value(v: &SafetyCondition) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::record(
        safety_payload_type(),
        vec![
            value_field(
                "inhibited",
                leaf_value("value/bool", InfoBool::new(v.inhibited).encode().to_vec())?,
            ),
            value_field(
                "motion_available",
                leaf_value(
                    "value/bool",
                    InfoBool::new(v.motion_available).encode().to_vec(),
                )?,
            ),
        ],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn capability_payload_value(
    v: &CapabilityCondition,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    StructuredInfoValue::record(
        capability_payload_type(),
        vec![value_field(
            "available",
            leaf_value("value/bool", InfoBool::new(v.available).encode().to_vec())?,
        )],
    )
    .map_err(|_| HomeostasisRefusal::EncodingCapacity)
}
fn power_observation_value(
    value: &conduit_human::SourceObservation<PowerCondition>,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    observation_value(value, "PetePowerObservation", |v| {
        StructuredInfoValue::record(
            StructuredInfoType::record(
                kind_id("observation/energy-reserve-and-charging@1"),
                vec![
                    field("charging", leaf("value/bool")),
                    field("reserve_permille", leaf("value/count")),
                ],
            )
            .unwrap(),
            vec![
                value_field(
                    "charging",
                    leaf_value("value/bool", InfoBool::new(v.charging).encode().to_vec())?,
                ),
                value_field(
                    "reserve_permille",
                    count_value(u64::from(v.reserve_permille))?,
                ),
            ],
        )
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
    })
}
fn thermal_observation_value(
    value: &conduit_human::SourceObservation<ThermalCondition>,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    observation_value(value, "PeteThermalObservation", |v| {
        StructuredInfoValue::record(
            StructuredInfoType::record(
                kind_id("observation/temperature-milli-celsius@1"),
                vec![field("milli_celsius", leaf("value/scalar"))],
            )
            .unwrap(),
            vec![value_field(
                "milli_celsius",
                scalar_value(i64::from(v.milli_celsius))?,
            )],
        )
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
    })
}
fn pressure_observation_value(
    value: &conduit_human::SourceObservation<PressureCondition>,
    name: &str,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    observation_value(value, name, |v| {
        StructuredInfoValue::record(
            StructuredInfoType::record(
                kind_id("observation/pressure-permille@1"),
                vec![field("pressure_permille", leaf("value/count"))],
            )
            .unwrap(),
            vec![value_field(
                "pressure_permille",
                count_value(u64::from(v.pressure_permille))?,
            )],
        )
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
    })
}
fn safety_observation_value(
    value: &conduit_human::SourceObservation<SafetyCondition>,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    observation_value(value, "PeteMotionSafetyObservation", |v| {
        StructuredInfoValue::record(
            StructuredInfoType::record(
                kind_id("pete/motion-safety-condition@1"),
                vec![
                    field("inhibited", leaf("value/bool")),
                    field("motion_available", leaf("value/bool")),
                ],
            )
            .unwrap(),
            vec![
                value_field(
                    "inhibited",
                    leaf_value("value/bool", InfoBool::new(v.inhibited).encode().to_vec())?,
                ),
                value_field(
                    "motion_available",
                    leaf_value(
                        "value/bool",
                        InfoBool::new(v.motion_available).encode().to_vec(),
                    )?,
                ),
            ],
        )
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
    })
}
fn capability_observation_value(
    value: &conduit_human::SourceObservation<CapabilityCondition>,
) -> Result<StructuredInfoValue, HomeostasisRefusal> {
    observation_value(value, "PeteImportantCapabilityObservation", |v| {
        StructuredInfoValue::record(
            StructuredInfoType::record(
                kind_id("observation/capability-availability@1"),
                vec![field("available", leaf("value/bool"))],
            )
            .unwrap(),
            vec![value_field(
                "available",
                leaf_value("value/bool", InfoBool::new(v.available).encode().to_vec())?,
            )],
        )
        .map_err(|_| HomeostasisRefusal::EncodingCapacity)
    })
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
