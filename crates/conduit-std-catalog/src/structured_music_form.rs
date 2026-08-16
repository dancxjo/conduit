//! Structured authoring contracts for the portable breadboard instrument Form.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal, StructuredConfigurationValue, StructuredFieldType,
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue, StructuredVariantCase,
    MUSIC_CONTROL_INFO_ID, MUSIC_NOTE_INFO_ID,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const INSTRUMENT_CONTROL_TYPE: &str = "InstrumentControl";
pub const INSTRUMENT_MAPPING_TYPE: &str = "InstrumentMapping";
pub const INSTRUMENT_MAP_KIND: &str = "music/instrument-map";
pub const INSTRUMENT_MAP_REVISION: &str = "conduit.std/music-instrument-map@1";

pub fn instrument_mapping_type() -> StructuredInfoType {
    let count = leaf("value/count@1");
    StructuredInfoType::record(
        kind_id("music/instrument-mapping@1"),
        vec![
            field("expression_control", count.clone()),
            field("modulation_control", count.clone()),
            field(
                "pitches",
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
        .insert(KindSignature {
            kind: INSTRUMENT_MAP_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "mapping".into(),
                value_type: INSTRUMENT_MAPPING_TYPE.into(),
                default: None,
            }],
        })
        .map_err(|error| error.to_string())?;

    let mapping_profile = mapping
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(INSTRUMENT_MAP_KIND),
            kind_contract_revision: KindContractRevision::from(INSTRUMENT_MAP_REVISION),
            inputs: vec![PortDescriptor {
                port_id: port_id("input"),
                value_kind: control
                    .profile()
                    .map_err(|error| alloc::format!("{error:?}"))?
                    .value_kind()
                    .clone(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Flow { closes: true },
            }],
            outputs: vec![
                flow_port("notes", MUSIC_NOTE_INFO_ID, PortDirection::Output),
                flow_port("controls", MUSIC_CONTROL_INFO_ID, PortDirection::Output),
            ],
            configuration: vec![ConfigurationField {
                key: "mapping".into(),
                default_value: ConfigurationValue::Structured(default_mapping_configuration(
                    &mapping,
                    mapping_profile.value_kind(),
                )?),
                validation: ConfigurationRule::Structured {
                    profile: mapping_profile.value_kind().clone(),
                },
            }],
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn default_mapping_configuration(
    mapping: &StructuredInfoType,
    profile: &KindId,
) -> Result<StructuredConfigurationValue, String> {
    let count = leaf("value/count@1");
    let pitches_type = StructuredInfoType::collection(count.clone(), Some(8)).unwrap();
    let pitches = StructuredInfoValue::collection(
        pitches_type,
        [60_u64, 62, 64, 65, 67, 69, 71, 72]
            .into_iter()
            .map(|value| count_value(&count, value))
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| alloc::format!("{error:?}"))?;
    let value = StructuredInfoValue::record(
        mapping.clone(),
        vec![
            value_field("expression_control", count_value(&count, 1)?),
            value_field("modulation_control", count_value(&count, 0)?),
            value_field("pitches", pitches),
            value_field("sustain_button", count_value(&count, 8)?),
        ],
    )
    .map_err(|error| alloc::format!("{error:?}"))?;
    StructuredConfigurationValue::new(
        profile.clone(),
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
