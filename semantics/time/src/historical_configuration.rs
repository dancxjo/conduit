//! Complete checked configuration for a bounded typed-history operation.

use conduit_core::{ConfigurationEntry, ConfigurationValue, KindId, TemporalScale};

use crate::{BoundedHistoricalTimeline, HistoricalOverflowPolicy, HistoricalTimelineRefusal};

pub const HISTORICAL_TIME_SCALE_SECONDS: &str = "seconds";
pub const HISTORICAL_TIME_SCALE_MILLISECONDS: &str = "milliseconds";
pub const HISTORICAL_TIME_SCALE_MICROSECONDS: &str = "microseconds";
pub const HISTORICAL_TIME_SCALE_NANOSECONDS: &str = "nanoseconds";
pub const HISTORICAL_OVERFLOW_REFUSE: &str = "refuse";
pub const HISTORICAL_OVERFLOW_EVICT_OLDEST_WITH_GAP: &str = "evict-oldest-with-gap";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalConfigurationRefusal {
    Missing(&'static str),
    WrongType(&'static str),
    UnknownTimeScale,
    UnknownOverflowPolicy,
    Timeline(HistoricalTimelineRefusal),
}

pub fn historical_timeline_from_configuration(
    entries: &[ConfigurationEntry],
) -> Result<BoundedHistoricalTimeline, HistoricalConfigurationRefusal> {
    let text = |key: &'static str| -> Result<&str, HistoricalConfigurationRefusal> {
        match &find(entries, key)?.value {
            ConfigurationValue::Text(value) => Ok(value),
            _ => Err(HistoricalConfigurationRefusal::WrongType(key)),
        }
    };
    let count = |key: &'static str| -> Result<u64, HistoricalConfigurationRefusal> {
        match find(entries, key)?.value {
            ConfigurationValue::U64(value) => Ok(value),
            _ => Err(HistoricalConfigurationRefusal::WrongType(key)),
        }
    };
    let scale = match text("time-scale")? {
        HISTORICAL_TIME_SCALE_SECONDS => TemporalScale::Seconds,
        HISTORICAL_TIME_SCALE_MILLISECONDS => TemporalScale::Milliseconds,
        HISTORICAL_TIME_SCALE_MICROSECONDS => TemporalScale::Microseconds,
        HISTORICAL_TIME_SCALE_NANOSECONDS => TemporalScale::Nanoseconds,
        _ => return Err(HistoricalConfigurationRefusal::UnknownTimeScale),
    };
    let overflow = match text("overflow-policy")? {
        HISTORICAL_OVERFLOW_REFUSE => HistoricalOverflowPolicy::Refuse,
        HISTORICAL_OVERFLOW_EVICT_OLDEST_WITH_GAP => HistoricalOverflowPolicy::EvictOldestWithGap,
        _ => return Err(HistoricalConfigurationRefusal::UnknownOverflowPolicy),
    };
    let maximum_entries = usize::try_from(count("maximum-entries")?).map_err(|_| {
        HistoricalConfigurationRefusal::Timeline(HistoricalTimelineRefusal::InvalidLimits)
    })?;
    BoundedHistoricalTimeline::new(
        KindId::from(text("value-profile")?),
        text("clock-basis")?,
        scale,
        maximum_entries,
        count("maximum-referenced-bytes")?,
        overflow,
        count("first-sequence")?,
    )
    .map_err(HistoricalConfigurationRefusal::Timeline)
}

fn find<'a>(
    entries: &'a [ConfigurationEntry],
    key: &'static str,
) -> Result<&'a ConfigurationEntry, HistoricalConfigurationRefusal> {
    entries
        .iter()
        .find(|entry| entry.key == key)
        .ok_or(HistoricalConfigurationRefusal::Missing(key))
}

#[cfg(feature = "form-catalog")]
pub fn historical_timeline_kind_projection() -> conduit_form::KindProjection {
    use alloc::{string::ToString, vec};
    use conduit_core::{
        kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
        StructuredInfoType, MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES,
        MAXIMUM_TEMPORAL_IDENTITY_BYTES,
    };
    use conduit_form::{KindConfigurationField, KindConfigurationRule};
    let value_kind = |identity| {
        StructuredInfoType::leaf(kind_id(identity))
            .expect("reviewed history value identity")
            .profile()
            .expect("reviewed history value profile")
            .value_kind()
            .clone()
    };
    conduit_form::KindProjection {
        kind_id: kind_id(crate::HISTORICAL_TIMELINE_KIND),
        kind_contract_revision: KindIdentity::from(crate::HISTORICAL_TIMELINE_CONTRACT_REVISION),
        inputs: alloc::vec![PortDescriptor {
            port_id: port_id("command"),
            value_kind: value_kind(crate::HISTORICAL_TIMELINE_COMMAND_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: alloc::vec![PortDescriptor {
            port_id: port_id("timeline"),
            value_kind: value_kind("history/typed-timeline@1"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: vec![
            KindConfigurationField {
                key: "value-profile".to_string(),
                default_value: ConfigurationValue::Text("value/text".to_string()),
                rule: KindConfigurationRule::TextBytes {
                    maximum: MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES as u32,
                },
            },
            KindConfigurationField {
                key: "clock-basis".to_string(),
                default_value: ConfigurationValue::Text("history/event-clock".to_string()),
                rule: KindConfigurationRule::TextBytes {
                    maximum: MAXIMUM_TEMPORAL_IDENTITY_BYTES as u32,
                },
            },
            KindConfigurationField {
                key: "time-scale".to_string(),
                default_value: ConfigurationValue::Text(
                    HISTORICAL_TIME_SCALE_MILLISECONDS.to_string(),
                ),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec![
                        HISTORICAL_TIME_SCALE_SECONDS.to_string(),
                        HISTORICAL_TIME_SCALE_MILLISECONDS.to_string(),
                        HISTORICAL_TIME_SCALE_MICROSECONDS.to_string(),
                        HISTORICAL_TIME_SCALE_NANOSECONDS.to_string(),
                    ],
                },
            },
            KindConfigurationField {
                key: "maximum-entries".to_string(),
                default_value: ConfigurationValue::U64(16),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: crate::MAXIMUM_HISTORICAL_TIMELINE_ENTRIES as u64,
                },
            },
            KindConfigurationField {
                key: "maximum-referenced-bytes".to_string(),
                default_value: ConfigurationValue::U64(1_048_576),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: crate::MAXIMUM_HISTORICAL_REFERENCED_BYTES,
                },
            },
            KindConfigurationField {
                key: "overflow-policy".to_string(),
                default_value: ConfigurationValue::Text(HISTORICAL_OVERFLOW_REFUSE.to_string()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec![
                        HISTORICAL_OVERFLOW_REFUSE.to_string(),
                        HISTORICAL_OVERFLOW_EVICT_OLDEST_WITH_GAP.to_string(),
                    ],
                },
            },
            KindConfigurationField {
                key: "first-sequence".to_string(),
                default_value: ConfigurationValue::U64(0),
                rule: KindConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: u64::MAX - 1,
                },
            },
        ],
    }
}

#[cfg(feature = "form-catalog")]
pub fn historical_timeline_semantic_contract() -> conduit_core::Kind {
    use conduit_core::{kind_id, CapabilityLimits, FrontStartupParameter};

    let definition = historical_timeline_kind_projection();
    conduit_core::Kind {
        startup_parameters: [
            ("value-profile", "value/text"),
            ("clock-basis", "value/text"),
            ("time-scale", "value/text"),
            ("maximum-entries", "value/count"),
            ("maximum-referenced-bytes", "value/count"),
            ("overflow-policy", "value/text"),
            ("first-sequence", "value/count"),
        ]
        .into_iter()
        .map(|(name, value_type)| FrontStartupParameter {
            name: name.into(),
            value_type: kind_id(value_type),
            has_default: true,
        })
        .collect(),
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}
