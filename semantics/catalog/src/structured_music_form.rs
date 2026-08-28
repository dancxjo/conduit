//! Structured authoring contracts for the portable breadboard instrument Form.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_audio::{MUSIC_CONTROL_INFO_ID, MUSIC_NOTE_INFO_ID};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue, StructuredVariantCase,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const INSTRUMENT_CONTROL_TYPE: &str = "InstrumentControl";
pub const INSTRUMENT_MAPPING_TYPE: &str = "InstrumentMapping";
pub const INSTRUMENT_MAP_KIND: &str = "music/instrument-map";
pub const INSTRUMENT_MAP_REVISION: &str = "conduit.std/music-instrument-map@1";
pub const BEAT_REFERENCE_TYPE: &str = "BeatReference";
pub const TIMING_FEEDBACK_TYPE: &str = "TimingFeedback";
pub const RHYTHM_COMPARE_KIND: &str = "music/rhythm-compare";
pub const RHYTHM_COMPARE_REVISION: &str = "conduit.std/music-rhythm-compare@1";
pub const RHYTHM_MAXIMUM_PENDING_BEATS: u16 = 16;

pub fn beat_reference_type() -> StructuredInfoType {
    let count = leaf("value/count@1");
    StructuredInfoType::record(
        kind_id("music/beat-reference@1"),
        vec![
            field("beat", count.clone()),
            field("expected_time_micros", count),
        ],
    )
    .unwrap()
}

pub fn timing_feedback_type() -> StructuredInfoType {
    let count = leaf("value/count@1");
    StructuredInfoType::record(
        kind_id("music/timing-feedback@1"),
        vec![
            field("beat", count.clone()),
            field("classification", leaf("music/timing-classification@1")),
            field("delta_micros", leaf("time/signed-microseconds@1")),
            field("expected_time_micros", count.clone()),
            field("observed", leaf("value/boolean@1")),
            field("observed_time_micros", count),
            field("recovery_state", leaf("music/recovery-state@1")),
        ],
    )
    .unwrap()
}

pub fn instrument_mapping_type() -> StructuredInfoType {
    let count = leaf("value/count@1");
    StructuredInfoType::record(
        kind_id("music/instrument-mapping@1"),
        vec![
            field("expression_control", count.clone()),
            field("modulation_control", count.clone()),
            field(
                "pitch_millihertz",
                StructuredInfoType::collection(count.clone(), Some(8)).unwrap(),
            ),
            field("sustain_button", count),
        ],
    )
    .unwrap()
}

pub fn instrument_control_type() -> StructuredInfoType {
    let count = leaf("value/count@1");
    let boolean = leaf("value/boolean@1");
    let button = StructuredInfoType::record(
        kind_id("input/button-event@1"),
        vec![
            field("down", boolean),
            field("event_time_micros", count.clone()),
            field("index", count.clone()),
            field("occurrence", count.clone()),
        ],
    )
    .unwrap();
    let analog = StructuredInfoType::record(
        kind_id("input/analog-event@1"),
        vec![
            field("event_time_micros", count.clone()),
            field("index", count.clone()),
            field("value", count),
        ],
    )
    .unwrap();
    StructuredInfoType::variant(
        kind_id("input/instrument-control@1"),
        vec![
            StructuredVariantCase::new("analog", analog).unwrap(),
            StructuredVariantCase::new("button", button).unwrap(),
        ],
    )
    .unwrap()
}

pub fn install_structured_music_form_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    let mapping = instrument_mapping_type();
    let control = instrument_control_type();
    startup
        .insert_structured_type(INSTRUMENT_MAPPING_TYPE, mapping.clone())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(INSTRUMENT_CONTROL_TYPE, control.clone())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(BEAT_REFERENCE_TYPE, beat_reference_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(TIMING_FEEDBACK_TYPE, timing_feedback_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: INSTRUMENT_MAP_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "mapping".into(),
                value_type: INSTRUMENT_MAPPING_TYPE.into(),
                default: None,
            }],
        })
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: RHYTHM_COMPARE_KIND.into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "target-offset-micros".into(),
                    value_type: "Scalar".into(),
                    default: Some("0".into()),
                },
                StartupParameterSignature {
                    name: "tolerance-micros".into(),
                    value_type: "Count".into(),
                    default: Some("30000".into()),
                },
            ],
        })
        .map_err(|error| error.to_string())?;

    profile
        .insert(instrument_map_definition()?)
        .map_err(|error| error.to_string())?;
    profile
        .insert(rhythm_compare_definition())
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn rhythm_compare_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(RHYTHM_COMPARE_KIND),
        kind_contract_revision: KindContractRevision::from(RHYTHM_COMPARE_REVISION),
        inputs: vec![
            flow_port("performance", MUSIC_NOTE_INFO_ID, PortDirection::Input),
            structured_flow_port("reference", &beat_reference_type(), PortDirection::Input),
        ],
        outputs: vec![structured_flow_port(
            "feedback",
            &timing_feedback_type(),
            PortDirection::Output,
        )],
        configuration: vec![
            ConfigurationField {
                key: "target-offset-micros".into(),
                default_value: ConfigurationValue::I64(0),
                validation: ConfigurationRule::I64Range {
                    minimum: -60_000_000,
                    maximum: 60_000_000,
                },
            },
            ConfigurationField {
                key: "tolerance-micros".into(),
                default_value: ConfigurationValue::U64(30_000),
                validation: ConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: 1_000_000,
                },
            },
        ],
    }
}

pub fn instrument_map_definition() -> Result<KindDefinition, String> {
    let control_kind = instrument_control_type()
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?
        .value_kind()
        .clone();
    let mapping_kind = instrument_mapping_type()
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?
        .value_kind()
        .clone();
    Ok(KindDefinition {
        kind_id: kind_id(INSTRUMENT_MAP_KIND),
        kind_contract_revision: KindContractRevision::from(INSTRUMENT_MAP_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: control_kind,
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![
            flow_port("notes", MUSIC_NOTE_INFO_ID, PortDirection::Output),
            flow_port("controls", MUSIC_CONTROL_INFO_ID, PortDirection::Output),
        ],
        configuration: vec![ConfigurationField {
            key: "mapping".into(),
            default_value: ConfigurationValue::Structured(
                default_instrument_mapping_configuration()?,
            ),
            validation: ConfigurationRule::Structured {
                profile: mapping_kind,
            },
        }],
    })
}

pub fn default_instrument_mapping_configuration() -> Result<StructuredConfigurationValue, String> {
    let mapping = instrument_mapping_type();
    let profile = mapping
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?
        .value_kind()
        .clone();
    let count = leaf("value/count@1");
    let pitches_type = StructuredInfoType::collection(count.clone(), Some(8)).unwrap();
    let pitches = StructuredInfoValue::collection(
        pitches_type,
        [
            261_626_u64,
            293_665,
            329_628,
            349_228,
            391_995,
            440_000,
            493_883,
            523_251,
        ]
        .into_iter()
        .map(|value| count_value(&count, value))
        .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| alloc::format!("{error:?}"))?;
    let value = StructuredInfoValue::record(
        mapping,
        vec![
            value_field("expression_control", count_value(&count, 1)?),
            value_field("modulation_control", count_value(&count, 0)?),
            value_field("pitch_millihertz", pitches),
            value_field("sustain_button", count_value(&count, 8)?),
        ],
    )
    .map_err(|error| alloc::format!("{error:?}"))?;
    StructuredConfigurationValue::new(
        profile,
        value
            .canonical_bytes()
            .map_err(|error| alloc::format!("{error:?}"))?,
    )
    .ok_or_else(|| "default instrument mapping exceeds configuration bounds".into())
}

fn count_value(value_type: &StructuredInfoType, value: u64) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::leaf(value_type.clone(), value.to_string().into_bytes())
        .map_err(|error| alloc::format!("{error:?}"))
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).unwrap()
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).unwrap()
}

fn value_field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).unwrap()
}

fn flow_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

fn structured_flow_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}
