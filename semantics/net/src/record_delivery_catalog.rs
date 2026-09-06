//! Ordinary Form contract for correlated record-delivery observations.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

pub const RECORD_DELIVERY_STATUS_KIND: &str = "record/delivery-status";
pub const RECORD_DELIVERY_STATUS_CONTRACT_REVISION: &str = "conduit.net/record-delivery-status@1";

pub fn install_record_delivery_status_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup
        .insert_structured_type("RecordDeliveryObservation", delivery_observation_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type("RecordDeliveryStatus", delivery_status_type())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: RECORD_DELIVERY_STATUS_KIND.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(record_delivery_status_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn delivery_observation_type() -> StructuredInfoType {
    leaf("record/delivery-observation@1")
}

pub fn delivery_status_type() -> StructuredInfoType {
    leaf("record/delivery-status@1")
}

pub fn record_delivery_status_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(RECORD_DELIVERY_STATUS_KIND),
        kind_contract_revision: KindContractRevision::from(
            RECORD_DELIVERY_STATUS_CONTRACT_REVISION,
        ),
        inputs: vec![port(
            "observation",
            &delivery_observation_type(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "status",
            &delivery_status_type(),
            PortDirection::Output,
        )],
        configuration: vec![],
    }
}

fn leaf(identity: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(identity)).expect("the reviewed record identity is finite")
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}
