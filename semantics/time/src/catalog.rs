use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{kind_id, ConfigurationValue, KindContractRevision};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

use crate::{
    historical_timeline_kind_definition, replay_control_kind_definition,
    replay_source_kind_definition, tick_outputs, HISTORICAL_TIMELINE_KIND, MAX_TICK_COUNT,
    REPLAY_SOURCE_KIND, TICK_CONTRACT_REVISION, TICK_KIND, TIME_EVERY_CONTRACT_REVISION,
    TIME_EVERY_KIND,
};

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
        outputs: tick_outputs(),
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
                default: Some(crate::REPLAY_MODE_ORIGINAL_TIMING.to_string()),
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
                default: Some("value/text@1".to_string()),
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
                default: Some("refuse".to_string()),
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
    }
}
