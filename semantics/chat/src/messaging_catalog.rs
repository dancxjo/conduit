//! Canonical Form catalog for portable messaging semantics.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
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
    for contract in messaging_semantic_contracts() {
        insert_kind(startup, profile, contract)?;
    }
    Ok(())
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    contract: Kind,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

pub fn messaging_semantic_contracts() -> Vec<Kind> {
    vec![
        messaging_semantic_contract(
            MESSAGING_MESSAGE_KIND,
            vec![],
            vec![
                port("message", &portable_message_type(), PortDirection::Output),
                port("request", &delivery_request_type(), PortDirection::Output),
            ],
        ),
        messaging_semantic_contract(
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
        ),
    ]
}

fn messaging_semantic_contract(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(MESSAGING_REVISION),
        inputs,
        outputs,
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messaging_semantic_contracts_own_exact_ports_and_finite_bounds() {
        let contracts = messaging_semantic_contracts();
        assert_eq!(contracts.len(), 2);
        let message = &contracts[0];
        let delivery = &contracts[1];
        assert_eq!(message.kind_id.as_str(), MESSAGING_MESSAGE_KIND);
        assert_eq!(message.inputs.len(), 0);
        assert_eq!(message.outputs.len(), 2);
        assert_eq!(delivery.kind_id.as_str(), MESSAGING_DELIVERY_KIND);
        assert_eq!(delivery.inputs.len(), 1);
        assert_eq!(delivery.outputs.len(), 2);
        for contract in contracts {
            assert_eq!(contract.kind_contract_revision.as_str(), MESSAGING_REVISION);
            assert_eq!(contract.limits.max_active_instances, 4);
            assert_eq!(contract.limits.max_queue_items, 4);
            assert_eq!(
                contract.limits.max_queue_bytes,
                (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32
            );
        }
    }
}
