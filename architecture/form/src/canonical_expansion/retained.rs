use crate::{
    CanonicalExpansionDiagnostic, CanonicalStartupValue, CheckedGear, CheckedRetainedValue,
};
use alloc::{vec, vec::Vec};
use conduit_core::{GearId, KindId, PortDescriptor};

pub(super) fn initialized_structured_state(
    retained: &CheckedRetainedValue,
    gear_id: GearId,
) -> Result<Option<(CheckedGear, PortDescriptor, PortDescriptor)>, CanonicalExpansionDiagnostic> {
    if retained.optional {
        return Ok(None);
    }
    let Some(CanonicalStartupValue::Structured(initial)) = retained.initial.as_ref() else {
        return Ok(None);
    };
    let concrete = initial.try_concrete().ok_or_else(|| {
        CanonicalExpansionDiagnostic::new(
            "CND-FRM-039",
            "retained structured initializer remains unresolved".into(),
        )
    })?;
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
    let maximum_bytes = retained.maximum_bytes.unwrap_or(encoded_bytes);
    if maximum_bytes < encoded_bytes {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-041",
            format!(
                "KEEP structured bound is smaller than its {encoded_bytes}-byte canonical encoding"
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
    let gear = CheckedGear {
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
        terminal_transduction: None,
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
