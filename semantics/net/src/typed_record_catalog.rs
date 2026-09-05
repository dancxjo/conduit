//! Ordinary Form contracts for transport-neutral typed-record framing.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

use crate::{FRAMED_TYPED_RECORD_INFO_ID, TYPED_RECORD_INFO_ID};

pub const TYPED_RECORD_FRAME_KIND: &str = "record/frame-typed";
pub const TYPED_RECORD_DEFRAME_KIND: &str = "record/deframe-typed";
pub const TYPED_RECORD_CONTRACT_REVISION: &str = "conduit.net/typed-record-frame@1";

pub fn install_typed_record_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type("TypedRecord", typed_record_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type("FramedTypedRecord", framed_typed_record_type())
        .map_err(|error| error.to_string())?;
    for definition in typed_record_definitions() {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().to_string(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn typed_record_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(TYPED_RECORD_INFO_ID))
        .expect("the typed-record identity is finite")
}

pub fn framed_typed_record_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(FRAMED_TYPED_RECORD_INFO_ID))
        .expect("the framed typed-record identity is finite")
}

fn typed_record_definitions() -> [KindDefinition; 2] {
    let record = typed_record_type();
    let frame = framed_typed_record_type();
    [
        KindDefinition {
            kind_id: kind_id(TYPED_RECORD_FRAME_KIND),
            kind_contract_revision: KindContractRevision::from(TYPED_RECORD_CONTRACT_REVISION),
            inputs: vec![port("record", &record, PortDirection::Input)],
            outputs: vec![port("frame", &frame, PortDirection::Output)],
            configuration: vec![],
        },
        KindDefinition {
            kind_id: kind_id(TYPED_RECORD_DEFRAME_KIND),
            kind_contract_revision: KindContractRevision::from(TYPED_RECORD_CONTRACT_REVISION),
            inputs: vec![port("frame", &frame, PortDirection::Input)],
            outputs: vec![port("record", &record, PortDirection::Output)],
            configuration: vec![],
        },
    ]
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
