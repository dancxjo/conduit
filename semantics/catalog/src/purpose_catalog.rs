//! Portable typed seams for finite Body purpose and deterministic readiness.

use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};
use conduit_form::{KindProjection, KindSignature};

pub const PURPOSE_READINESS_KIND: &str = "purpose/fulfillment-readiness";
pub const PURPOSE_STATE_TYPE: &str = "PurposeState";
pub const FULFILLMENT_READINESS_TYPE: &str = "FulfillmentReadiness";

pub fn purpose_state_type() -> StructuredInfoType {
    conduit_purpose::purpose_state_type()
}

pub fn fulfillment_readiness_type() -> StructuredInfoType {
    conduit_purpose::fulfillment_readiness_type()
}

pub fn install_purpose_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in [
        (PURPOSE_STATE_TYPE, purpose_state_type()),
        (FULFILLMENT_READINESS_TYPE, fulfillment_readiness_type()),
    ] {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    startup
        .insert(KindSignature {
            kind: PURPOSE_READINESS_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: kind_id(PURPOSE_READINESS_KIND),
            kind_contract_revision: KindIdentity::from("conduit.purpose/fulfillment-readiness@1"),
            inputs: vec![port("purpose", &purpose_state_type(), PortDirection::Input)],
            outputs: vec![port(
                "readiness",
                &fulfillment_readiness_type(),
                PortDirection::Output,
            )],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Current,
        abnormal_kind: None,
    }
}
