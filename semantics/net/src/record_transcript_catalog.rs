//! Ordinary Form contract for finite typed-record transcript retention.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

use crate::{
    framed_typed_record_type, MAXIMUM_RECORD_TRANSCRIPT_BYTES, MAXIMUM_RECORD_TRANSCRIPT_ITEMS,
    MAXIMUM_TYPED_RECORD_FRAME_BYTES, TYPED_RECORD_FRAME_HEADER_BYTES,
};

pub const RECORD_TRANSCRIPT_KIND: &str = "record/bounded-transcript";
pub const RECORD_TRANSCRIPT_CONTRACT_REVISION: &str = "conduit.net/record-transcript@1";

pub fn install_record_transcript_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type("RecordTerminalEvent", terminal_event_type())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: RECORD_TRANSCRIPT_KIND.to_string(),
        startup_parameters: vec![
            parameter("maximum-items", "16"),
            parameter(
                "maximum-frame-bytes",
                &MAXIMUM_TYPED_RECORD_FRAME_BYTES.to_string(),
            ),
            parameter(
                "maximum-retained-bytes",
                &MAXIMUM_RECORD_TRANSCRIPT_BYTES.to_string(),
            ),
        ],
    })?;
    profile
        .insert(record_transcript_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn record_transcript_kind_definition() -> KindDefinition {
    let frame = framed_typed_record_type();
    let terminal = terminal_event_type();
    KindDefinition {
        kind_id: kind_id(RECORD_TRANSCRIPT_KIND),
        kind_contract_revision: KindContractRevision::from(RECORD_TRANSCRIPT_CONTRACT_REVISION),
        inputs: vec![
            port("sent", &frame, PortDirection::Input),
            port("received", &frame, PortDirection::Input),
            port("terminal", &terminal, PortDirection::Input),
        ],
        outputs: vec![
            port("retained-sent", &frame, PortDirection::Output),
            port("retained-received", &frame, PortDirection::Output),
            port("retained-terminal", &terminal, PortDirection::Output),
        ],
        configuration: vec![
            count_field(
                "maximum-items",
                16,
                1,
                MAXIMUM_RECORD_TRANSCRIPT_ITEMS as u64,
            ),
            count_field(
                "maximum-frame-bytes",
                MAXIMUM_TYPED_RECORD_FRAME_BYTES as u64,
                TYPED_RECORD_FRAME_HEADER_BYTES as u64,
                MAXIMUM_TYPED_RECORD_FRAME_BYTES as u64,
            ),
            count_field(
                "maximum-retained-bytes",
                MAXIMUM_RECORD_TRANSCRIPT_BYTES as u64,
                TYPED_RECORD_FRAME_HEADER_BYTES as u64,
                MAXIMUM_RECORD_TRANSCRIPT_BYTES as u64,
            ),
        ],
    }
}

pub fn terminal_event_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("record/terminal-event@1"))
        .expect("the terminal-event identity is finite")
}

fn parameter(name: &str, default: &str) -> StartupParameterSignature {
    StartupParameterSignature {
        name: name.to_string(),
        value_type: "Count".to_string(),
        default: Some(default.to_string()),
    }
}

fn count_field(key: &str, default: u64, minimum: u64, maximum: u64) -> ConfigurationField {
    ConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(default),
        validation: ConfigurationRule::U64Range { minimum, maximum },
    }
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}
