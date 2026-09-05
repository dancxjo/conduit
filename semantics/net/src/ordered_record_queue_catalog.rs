//! Ordinary Form contract for bounded ordered-record queueing.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

use crate::{
    framed_typed_record_type, MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS, MAXIMUM_TYPED_RECORD_FRAME_BYTES,
    TYPED_RECORD_FRAME_HEADER_BYTES,
};

pub const ORDERED_RECORD_QUEUE_KIND: &str = "record/ordered-send-queue";
pub const ORDERED_RECORD_QUEUE_CONTRACT_REVISION: &str = "conduit.net/ordered-record-queue@1";

pub fn install_ordered_record_queue_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(KindSignature {
        kind: ORDERED_RECORD_QUEUE_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "maximum-items".to_string(),
                value_type: "Count".to_string(),
                default: Some("4".to_string()),
            },
            StartupParameterSignature {
                name: "maximum-frame-bytes".to_string(),
                value_type: "Count".to_string(),
                default: Some(MAXIMUM_TYPED_RECORD_FRAME_BYTES.to_string()),
            },
        ],
    })?;
    profile
        .insert(ordered_record_queue_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn ordered_record_queue_kind_definition() -> KindDefinition {
    let frame = framed_typed_record_type();
    KindDefinition {
        kind_id: kind_id(ORDERED_RECORD_QUEUE_KIND),
        kind_contract_revision: KindContractRevision::from(ORDERED_RECORD_QUEUE_CONTRACT_REVISION),
        inputs: vec![port("frame", &frame, PortDirection::Input)],
        outputs: vec![port("queued", &frame, PortDirection::Output)],
        configuration: vec![
            ConfigurationField {
                key: "maximum-items".to_string(),
                default_value: ConfigurationValue::U64(4),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS as u64,
                },
            },
            ConfigurationField {
                key: "maximum-frame-bytes".to_string(),
                default_value: ConfigurationValue::U64(MAXIMUM_TYPED_RECORD_FRAME_BYTES as u64),
                validation: ConfigurationRule::U64Range {
                    minimum: TYPED_RECORD_FRAME_HEADER_BYTES as u64,
                    maximum: MAXIMUM_TYPED_RECORD_FRAME_BYTES as u64,
                },
            },
        ],
    }
}

fn port(
    name: &str,
    value_type: &conduit_core::StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}
