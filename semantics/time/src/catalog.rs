use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{
    kind_id, CapabilityLimits, ConfigurationValue, FaceStartupParameter, KindContractRevision,
    SemanticCapabilityContract,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

use crate::{
    historical_timeline_kind_definition, replay_control_kind_definition,
    replay_source_kind_definition, tick_outputs, time_every_outputs, HISTORICAL_TIMELINE_KIND,
    MAX_TICK_COUNT, PHASE_SYNCHRONIZE_KIND, PHASE_SYNCHRONIZE_REVISION,
    PULSE_OBSERVATION_VALUE_KIND, PULSE_OBSERVE_KIND, PULSE_OBSERVE_REVISION, REPLAY_SOURCE_KIND,
    RHYTHM_STATE_VALUE_KIND, TICK_CONTRACT_REVISION, TICK_KIND, TICK_VALUE_KIND,
    TIME_EVERY_CONTRACT_REVISION, TIME_EVERY_KIND,
};
use conduit_core::{port_id, PortDescriptor, PortDirection, PortTemporal};

pub fn tick_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(TICK_KIND),
        kind_contract_revision: KindContractRevision::from(TICK_CONTRACT_REVISION),
        inputs: alloc::vec::Vec::new(),
        outputs: tick_outputs(),
        configuration: vec![
            ConfigurationField {
                key: "count".to_string(),
                default_value: ConfigurationValue::U64(4),
                validation: ConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: MAX_TICK_COUNT,
                },
            },
            ConfigurationField {
                key: "period-ms".to_string(),
                default_value: ConfigurationValue::U64(1_000),
                validation: ConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: u64::MAX,
                },
            },
        ],
    }
}

pub fn time_every_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(TIME_EVERY_KIND),
        kind_contract_revision: KindContractRevision::from(TIME_EVERY_CONTRACT_REVISION),
        inputs: alloc::vec::Vec::new(),
        outputs: time_every_outputs(),
        configuration: vec![ConfigurationField {
            key: "freq".to_string(),
            default_value: ConfigurationValue::U64(1_000),
            validation: ConfigurationRule::DurationMillis {
                minimum: 0,
                maximum: u64::MAX,
            },
        }],
    }
}

pub fn install_tick_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: TICK_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "count".to_string(),
                value_type: "Count".to_string(),
                default: Some("4".to_string()),
            },
            StartupParameterSignature {
                name: "period-ms".to_string(),
                value_type: "Count".to_string(),
                default: Some("1000".to_string()),
            },
        ],
    })?;
    profile
        .insert(tick_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn install_time_every_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: TIME_EVERY_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "freq".to_string(),
            value_type: "Duration".to_string(),
            default: None,
        }],
    })?;
    profile
        .insert(time_every_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn install_rhythm_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: PULSE_OBSERVE_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "period-ms".into(),
            value_type: "Count".into(),
            default: Some("240".into()),
        }],
    })?;
    profile
        .insert(pulse_observe_kind_definition())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: PHASE_SYNCHRONIZE_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(phase_synchronize_kind_definition())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: crate::RHYTHM_STATE_SOURCE_KIND.into(),
        startup_parameters: [
            ("sequence", "0"),
            ("next-pulse-at-ms", "0"),
            ("period-ms", "240"),
            ("expected-peer-sequence", "0"),
        ]
        .into_iter()
        .map(|(name, default)| StartupParameterSignature {
            name: name.into(),
            value_type: "Count".into(),
            default: Some(default.into()),
        })
        .collect(),
    })?;
    profile
        .insert(rhythm_state_source_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn rhythm_state_source_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(crate::RHYTHM_STATE_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(crate::RHYTHM_STATE_SOURCE_REVISION),
        inputs: vec![],
        outputs: vec![flow_port(
            "state",
            RHYTHM_STATE_VALUE_KIND,
            PortDirection::Output,
        )],
        configuration: vec![
            count_field("sequence", 0, u32::MAX.into()),
            count_field("next-pulse-at-ms", 0, u32::MAX.into()),
            count_field(
                "period-ms",
                crate::MINIMUM_PERIOD_MS.into(),
                crate::MAXIMUM_PERIOD_MS.into(),
            ),
            count_field("expected-peer-sequence", 0, u32::MAX.into()),
        ],
    }
}

pub fn rhythm_state_source_semantic_contract() -> SemanticCapabilityContract {
    let definition = rhythm_state_source_kind_definition();
    SemanticCapabilityContract {
        startup_parameters: [
            "sequence",
            "next-pulse-at-ms",
            "period-ms",
            "expected-peer-sequence",
        ]
        .into_iter()
        .map(|name| FaceStartupParameter {
            name: name.into(),
            value_type: kind_id("value/count"),
            has_default: true,
        })
        .collect(),
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: crate::RHYTHM_STATE_ENCODED_LEN as u32,
        },
    }
}

fn count_field(key: &str, minimum: u64, maximum: u64) -> ConfigurationField {
    ConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(match key {
            "period-ms" => 240,
            _ => 0,
        }),
        validation: ConfigurationRule::U64Range { minimum, maximum },
    }
}

pub fn phase_synchronize_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(PHASE_SYNCHRONIZE_KIND),
        kind_contract_revision: KindContractRevision::from(PHASE_SYNCHRONIZE_REVISION),
        inputs: vec![
            flow_port("local", RHYTHM_STATE_VALUE_KIND, PortDirection::Input),
            flow_port("peer", PULSE_OBSERVATION_VALUE_KIND, PortDirection::Input),
        ],
        outputs: vec![flow_port(
            "updated",
            RHYTHM_STATE_VALUE_KIND,
            PortDirection::Output,
        )],
        configuration: vec![],
    }
}

pub fn phase_synchronize_semantic_contract() -> SemanticCapabilityContract {
    let definition = phase_synchronize_kind_definition();
    SemanticCapabilityContract {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: (crate::RHYTHM_STATE_ENCODED_LEN
                + crate::PULSE_OBSERVATION_ENCODED_LEN) as u32,
        },
    }
}

/// Nominal pulse period is semantic; each observation remains finite.
pub fn pulse_observe_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(PULSE_OBSERVE_KIND),
        kind_contract_revision: KindContractRevision::from(PULSE_OBSERVE_REVISION),
        inputs: vec![flow_port("tick", TICK_VALUE_KIND, PortDirection::Input)],
        outputs: vec![flow_port(
            "observation",
            PULSE_OBSERVATION_VALUE_KIND,
            PortDirection::Output,
        )],
        configuration: vec![ConfigurationField {
            key: "period-ms".into(),
            default_value: ConfigurationValue::U64(240),
            validation: ConfigurationRule::U64Range {
                minimum: crate::MINIMUM_PERIOD_MS.into(),
                maximum: crate::MAXIMUM_PERIOD_MS.into(),
            },
        }],
    }
}

pub fn pulse_observe_semantic_contract() -> SemanticCapabilityContract {
    let definition = pulse_observe_kind_definition();
    SemanticCapabilityContract {
        startup_parameters: vec![FaceStartupParameter {
            name: "period-ms".into(),
            value_type: kind_id("value/count"),
            has_default: true,
        }],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: crate::TICK_ENCODED_LEN,
        },
    }
}

fn flow_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Flow { closes: false },
    }
}

pub fn install_replay_control_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (name, identity) in [
        ("ReplayTimeline", "history/replay-timeline@1"),
        ("ReplayControl", "history/replay-control@1"),
        ("PlaybackTick", "time/playback-tick@1"),
        ("ReplayEvent", "history/replay-event@1"),
        ("ReplayState", "history/replay-state@1"),
    ] {
        startup
            .insert_structured_type(
                name,
                conduit_core::StructuredInfoType::leaf(kind_id(identity))
                    .expect("reviewed replay value identity"),
            )
            .map_err(|error| error.to_string())?;
    }
    startup.insert(KindSignature {
        kind: crate::REPLAY_CONTROL_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "mode".to_string(),
                value_type: "Text".to_string(),
                default: Some("\"original-timing\"".to_string()),
            },
            StartupParameterSignature {
                name: "rate-numerator".to_string(),
                value_type: "Count".to_string(),
                default: Some("1".to_string()),
            },
            StartupParameterSignature {
                name: "rate-denominator".to_string(),
                value_type: "Count".to_string(),
                default: Some("1".to_string()),
            },
            StartupParameterSignature {
                name: "maximum-duration-seconds".to_string(),
                value_type: "Count".to_string(),
                default: Some(crate::MAXIMUM_REPLAY_DURATION_SECONDS.to_string()),
            },
        ],
    })?;
    profile
        .insert(replay_control_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn install_historical_timeline_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (name, identity) in [
        (
            "HistoricalTimelineCommand",
            crate::HISTORICAL_TIMELINE_COMMAND_INFO_ID,
        ),
        ("HistoricalTypedTimeline", "history/typed-timeline@1"),
    ] {
        startup
            .insert_structured_type(
                name,
                conduit_core::StructuredInfoType::leaf(kind_id(identity))
                    .expect("reviewed history value identity"),
            )
            .map_err(|error| error.to_string())?;
    }
    startup.insert(KindSignature {
        kind: HISTORICAL_TIMELINE_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "value-profile".to_string(),
                value_type: "Text".to_string(),
                default: Some("\"value/text\"".to_string()),
            },
            StartupParameterSignature {
                name: "clock-basis".to_string(),
                value_type: "Text".to_string(),
                default: Some("\"history/event-clock\"".to_string()),
            },
            StartupParameterSignature {
                name: "time-scale".to_string(),
                value_type: "Text".to_string(),
                default: Some("\"milliseconds\"".to_string()),
            },
            StartupParameterSignature {
                name: "maximum-entries".to_string(),
                value_type: "Count".to_string(),
                default: Some("16".to_string()),
            },
            StartupParameterSignature {
                name: "maximum-referenced-bytes".to_string(),
                value_type: "Count".to_string(),
                default: Some("1048576".to_string()),
            },
            StartupParameterSignature {
                name: "overflow-policy".to_string(),
                value_type: "Text".to_string(),
                default: Some("\"refuse\"".to_string()),
            },
            StartupParameterSignature {
                name: "first-sequence".to_string(),
                value_type: "Count".to_string(),
                default: Some("0".to_string()),
            },
        ],
    })?;
    profile
        .insert(historical_timeline_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn install_replay_source_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_structured_type(
            "HistoricalRetentionGap",
            conduit_core::StructuredInfoType::leaf(kind_id("history/retention-gap@1"))
                .expect("reviewed retention-gap value identity"),
        )
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: REPLAY_SOURCE_KIND.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(replay_source_kind_definition())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definitions_preserve_exact_identities_and_bounds() {
        let tick = tick_kind_definition();
        assert_eq!(tick.kind_id.as_str(), TICK_KIND);
        assert_eq!(tick.outputs[0].value_kind.as_str(), crate::TICK_VALUE_KIND);
        assert_eq!(tick.kind_contract_revision.as_str(), TICK_CONTRACT_REVISION);
        let every = time_every_kind_definition();
        assert_eq!(every.kind_id.as_str(), TIME_EVERY_KIND);
        assert_eq!(
            every.kind_contract_revision.as_str(),
            TIME_EVERY_CONTRACT_REVISION
        );
        let pulse = pulse_observe_semantic_contract();
        assert_eq!(pulse.kind_id.as_str(), PULSE_OBSERVE_KIND);
        assert_eq!(pulse.startup_parameters.len(), 1);
        assert_eq!(pulse.startup_parameters[0].name, "period-ms");
        assert!(pulse.startup_parameters[0].has_default);
        assert_eq!(pulse.limits.max_active_instances, 8);
        assert_eq!(pulse.limits.max_queue_bytes, crate::TICK_ENCODED_LEN);
    }
}
