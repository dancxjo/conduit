//! Canonical Form catalog for portable messaging semantics.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    delivery_request_type, delivery_update_type, messaging_registered_types,
    notification_event_type, portable_message_type,
};

pub const MESSAGING_MESSAGE_KIND: &str = "messaging/message";
pub const MESSAGING_DELIVERY_KIND: &str = "messaging/deliver";
pub const MESSAGING_REVISION: &str = "conduit.std/messaging-delivery@2";

pub fn install_messaging_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in messaging_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        MESSAGING_MESSAGE_KIND,
        vec![],
        vec![
            port("message", &portable_message_type(), PortDirection::Output),
            port("request", &delivery_request_type(), PortDirection::Output),
        ],
    )?;
    insert_kind(
        startup,
        profile,
        MESSAGING_DELIVERY_KIND,
        vec![port(
            "request",
            &delivery_request_type(),
            PortDirection::Input,
        )],
        vec![
            port(
                "notification",
                &notification_event_type(),
                PortDirection::Output,
            ),
            port("update", &delivery_update_type(), PortDirection::Output),
        ],
    )
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(MESSAGING_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed messaging profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
