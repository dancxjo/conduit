//! Structured authoring contracts for the portable breadboard instrument Plot.

use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_audio::{
    BeatReference, InstrumentControl, InstrumentMapping, InstrumentPitchMillihertz, TimingFeedback,
    MUSIC_CONTROL_INFO_ID, MUSIC_NOTE_INFO_ID,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, FrontStartupParameter, Kind, KindId,
    KindIdentity, PortDescriptor, PortDirection, PortTemporal, StructuredConfigurationValue,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature,
    StartupParameterSignature,
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

pub fn rhythm_compare_semantic_contract() -> Kind {
    let definition = rhythm_compare_definition();
    Kind {
        startup_parameters: vec![
            startup("target-offset-micros", kind_id("value/scalar"), true),
            startup("tolerance-micros", kind_id("value/count"), true),
        ],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: definition.configuration,
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: RHYTHM_MAXIMUM_PENDING_BEATS,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(RHYTHM_MAXIMUM_PENDING_BEATS)
                * 3) as u32,
        },
    }
}

pub fn instrument_map_semantic_contract() -> Result<Kind, String> {
    let definition = instrument_map_definition()?;
    let mapping_kind = instrument_mapping_type()
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?
        .value_kind()
        .clone();
    Ok(Kind {
        startup_parameters: vec![startup("mapping", mapping_kind, false)],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: definition.configuration,
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 16,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    })
}

fn startup(name: &str, value_type: KindId, has_default: bool) -> FrontStartupParameter {
    FrontStartupParameter {
        name: name.into(),
        value_type,
        has_default,
    }
}

pub fn beat_reference_type() -> StructuredInfoType {
    BeatReference::semantic_type().expect("generated beat reference Type is checked")
}

pub fn timing_feedback_type() -> StructuredInfoType {
    TimingFeedback::semantic_type().expect("generated timing feedback Type is checked")
}

pub fn instrument_mapping_type() -> StructuredInfoType {
    InstrumentMapping::semantic_type().expect("generated instrument mapping Type is checked")
}

pub fn instrument_control_type() -> StructuredInfoType {
    InstrumentControl::semantic_type().expect("generated instrument control Type is checked")
}

pub fn install_structured_music_plot_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
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
        .insert_kind(instrument_map_semantic_contract()?)
        .map_err(|error| error.to_string())?;
    profile
        .insert_kind(rhythm_compare_semantic_contract())
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn rhythm_compare_definition() -> KindProjection {
    KindProjection {
        kind_id: kind_id(RHYTHM_COMPARE_KIND),
        kind_contract_revision: KindIdentity::from(RHYTHM_COMPARE_REVISION),
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
            KindConfigurationField {
                key: "target-offset-micros".into(),
                default_value: ConfigurationValue::I64(0),
                rule: KindConfigurationRule::I64Range {
                    minimum: -60_000_000,
                    maximum: 60_000_000,
                },
            },
            KindConfigurationField {
                key: "tolerance-micros".into(),
                default_value: ConfigurationValue::U64(30_000),
                rule: KindConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: 1_000_000,
                },
            },
        ],
    }
}

pub fn instrument_map_definition() -> Result<KindProjection, String> {
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
    Ok(KindProjection {
        kind_id: kind_id(INSTRUMENT_MAP_KIND),
        kind_contract_revision: KindIdentity::from(INSTRUMENT_MAP_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: control_kind,
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![
            flow_port("notes", MUSIC_NOTE_INFO_ID, PortDirection::Output),
            flow_port("controls", MUSIC_CONTROL_INFO_ID, PortDirection::Output),
        ],
        configuration: vec![KindConfigurationField {
            key: "mapping".into(),
            default_value: ConfigurationValue::Structured(
                default_instrument_mapping_configuration()?,
            ),
            rule: KindConfigurationRule::Structured {
                profile: mapping_kind,
            },
        }],
    })
}

pub fn default_instrument_mapping_configuration() -> Result<StructuredConfigurationValue, String> {
    let profile = instrument_mapping_type()
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?
        .value_kind()
        .clone();
    let pitches = InstrumentPitchMillihertz::new([
        261_626_u64,
        293_665,
        329_628,
        349_228,
        391_995,
        440_000,
        493_883,
        523_251,
    ])
    .map_err(|error| alloc::format!("{error:?}"))?;
    let value = InstrumentMapping::new(1, 0, pitches, 8)
        .and_then(NativeRustBinding::into_structured)
        .map_err(|error| alloc::format!("{error:?}"))?;
    StructuredConfigurationValue::new(
        profile,
        value
            .canonical_bytes()
            .map_err(|error| alloc::format!("{error:?}"))?,
    )
    .ok_or_else(|| "default instrument mapping exceeds configuration bounds".into())
}

fn flow_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
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
        abnormal_kind: None,
    }
}
