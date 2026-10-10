use crate::{
    CanonicalExpansionDiagnostic, CanonicalStartupValue, CheckedGear, CheckedRetainedValue,
};
use alloc::{vec, vec::Vec};
use conduit_core::{GearId, KindId, PortDescriptor};

pub(super) fn initialized_structured_state(
    retained: &CheckedRetainedValue,
    gear_id: GearId,
) -> Result<Option<(CheckedGear, PortDescriptor, PortDescriptor)>, CanonicalExpansionDiagnostic> {
    let concrete = if retained.optional {
        let payload_type = match retained.value_type.shape() {
            conduit_core::StructuredInfoTypeShape::Variant { cases, .. } => cases
                .iter()
                .find(|case| case.tag() == "some")
                .map(conduit_core::StructuredVariantCase::payload_type),
            _ => None,
        }
        .ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                "retained optional type is not canonical none|some(T)".into(),
            )
        })?;
        let (tag, payload) = match retained.initial.as_ref() {
            Some(CanonicalStartupValue::Structured(initial)) => (
                "some",
                initial.try_concrete().ok_or_else(|| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-039",
                        "retained optional initializer remains unresolved".into(),
                    )
                })?,
            ),
            Some(CanonicalStartupValue::Quantity(quantity)) => (
                "some",
                conduit_core::StructuredInfoValue::leaf(
                    payload_type.clone(),
                    quantity.value().encode().to_vec(),
                )
                .map_err(|_| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        "retained optional quantity initializer does not match its exact payload type"
                            .into(),
                    )
                })?,
            ),
            None => (
                "none",
                conduit_core::StructuredInfoValue::leaf(
                    conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                        conduit_core::EMPTY_INFO_ID,
                    ))
                    .map_err(|_| {
                        CanonicalExpansionDiagnostic::new(
                            "CND-FRM-041",
                            "canonical optional Empty payload is invalid".into(),
                        )
                    })?,
                    Vec::new(),
                )
                .map_err(|_| {
                    CanonicalExpansionDiagnostic::new(
                        "CND-FRM-041",
                        "canonical optional none payload is invalid".into(),
                    )
                })?,
            ),
            Some(_) => return Ok(None),
        };
        if tag == "some" && payload.value_type() != payload_type {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                "retained optional initializer has the wrong payload type".into(),
            ));
        }
        conduit_core::StructuredInfoValue::variant(retained.value_type.clone(), tag, payload)
            .map_err(|_| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    "retained optional initializer is not canonical none|some(T)".into(),
                )
            })?
    } else {
        let Some(CanonicalStartupValue::Structured(initial)) = retained.initial.as_ref() else {
            return Ok(None);
        };
        initial.try_concrete().ok_or_else(|| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-039",
                "retained structured initializer remains unresolved".into(),
            )
        })?
    };
    let canonical = concrete.canonical_bytes().map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            "retained structured initializer exceeds canonical bounds".into(),
        )
    })?;
    let encoded_bytes = u64::try_from(canonical.len()).map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            "retained structured storage bound exceeds Plan capacity".into(),
        )
    })?;
    let optional_value_bytes = if retained.optional {
        Some(
            canonical_optional_some_bytes(&retained.value_type)?
                .unwrap_or(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u64),
        )
    } else {
        None
    };
    let required_bytes = optional_value_bytes.unwrap_or(encoded_bytes);
    let maximum_bytes = retained.maximum_bytes.unwrap_or(required_bytes);
    if maximum_bytes < required_bytes {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            format!(
                "KEEP structured bound is smaller than its {required_bytes}-byte admitted value envelope"
            ),
        ));
    }
    let profile = retained.value_type.profile().map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            "retained structured type has no finite profile".into(),
        )
    })?;
    let value_kind = retained.value_kind.clone();
    let initial =
        conduit_core::StructuredConfigurationValue::new(profile.value_kind().clone(), canonical)
            .ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-041",
                    "retained structured initializer exceeds configuration bounds".into(),
                )
            })?;
    let input = PortDescriptor {
        port_id: conduit_core::port_id("next"),
        value_kind: value_kind.clone(),
        direction: conduit_core::PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: conduit_core::port_id("current"),
        value_kind: value_kind.clone(),
        direction: conduit_core::PortDirection::Output,
        temporal: conduit_core::PortTemporal::Current,
        abnormal_kind: None,
    };
    let gear = crate::checked_gear_from_parts! {
        gear_id,
        kind_id: KindId::from(conduit_core::STATE_VALUE_KIND),
        kind_contract_revision: conduit_core::KindIdentity::from(
            conduit_core::STATE_VALUE_REVISION,
        ),
        startup_parameters: vec![conduit_core::FrontStartupParameter {
            name: "initial".into(),
            value_type: profile.value_kind().clone(),
            has_default: false,
        }],
        shorthand: Some((input.port_id.clone(), output.port_id.clone())),
        inputs: vec![input.clone()],
        outputs: vec![output.clone()],
        semantic_contract: conduit_core::KindSemanticContract {
            configuration: vec![conduit_core::KindConfigurationField {
                key: "initial".into(),
                default_value: conduit_core::ConfigurationValue::Structured(initial.clone()),
                rule: conduit_core::KindConfigurationRule::Structured {
                    profile: profile.value_kind().clone(),
                },
            }],
            laws: Vec::new(),
        },
        terminal_transductions: Vec::new(),
        resource_ports: Vec::new(),
        configuration: vec![
            conduit_core::ConfigurationEntry {
                key: "initial".into(),
                value: conduit_core::ConfigurationValue::Structured(initial),
            },
            conduit_core::ConfigurationEntry {
                key: "retained-duration".into(),
                value: conduit_core::ConfigurationValue::Text(
                    match retained.duration {
                        crate::RetainedDuration::Step => "step",
                        crate::RetainedDuration::Play => "play",
                        crate::RetainedDuration::Wake => "wake",
                        crate::RetainedDuration::Boot => "boot",
                        crate::RetainedDuration::Body => "body",
                    }
                    .into(),
                ),
            },
            conduit_core::ConfigurationEntry {
                key: "maximum-bytes".into(),
                value: conduit_core::ConfigurationValue::U64(maximum_bytes),
            },
        ],
        pool_references: Vec::new(),
    };
    Ok(Some((gear, input, output)))
}

fn canonical_optional_some_bytes(
    optional: &conduit_core::StructuredInfoType,
) -> Result<Option<u64>, CanonicalExpansionDiagnostic> {
    let conduit_core::StructuredInfoTypeShape::Variant { cases, .. } = optional.shape() else {
        return Ok(None);
    };
    let Some(payload_type) = cases
        .iter()
        .find(|case| case.tag() == "some")
        .map(conduit_core::StructuredVariantCase::payload_type)
    else {
        return Ok(None);
    };
    let conduit_core::StructuredInfoTypeShape::Leaf(kind) = payload_type.shape() else {
        return Ok(None);
    };
    let encoded = match kind.as_str() {
        conduit_core::BOOL_INFO_ID => conduit_core::InfoBool::FALSE.encode().to_vec(),
        conduit_core::DISTANCE_INFO_ID => conduit_core::Quantity::new(0, conduit_core::Unit::Meter)
            .encode()
            .to_vec(),
        conduit_core::FREQUENCY_INFO_ID => {
            conduit_core::Quantity::new(0, conduit_core::Unit::Hertz)
                .encode()
                .to_vec()
        }
        _ => return Ok(None),
    };
    let payload =
        conduit_core::StructuredInfoValue::leaf(payload_type.clone(), encoded).map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                "canonical optional payload is invalid".into(),
            )
        })?;
    let some = conduit_core::StructuredInfoValue::variant(optional.clone(), "some", payload)
        .map_err(|_| {
            CanonicalExpansionDiagnostic::new(
                "CND-FRM-041",
                "canonical optional some value is invalid".into(),
            )
        })?;
    let bytes = some.canonical_bytes().map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            "canonical optional some value exceeds finite bounds".into(),
        )
    })?;
    u64::try_from(bytes.len()).map(Some).map_err(|_| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            "canonical optional bound exceeds Plan capacity".into(),
        )
    })
}
