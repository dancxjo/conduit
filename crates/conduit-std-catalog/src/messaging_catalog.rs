//! Canonical messaging Form catalog and finite deterministic provider offers.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, AuthorityContractId, AuthorityRequirement, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    delivery_request_type, delivery_update_type, messaging_registered_types,
    notification_event_type, portable_message_type,
};

pub const MESSAGING_MESSAGE_KIND: &str = "messaging/message";
pub const MESSAGING_DELIVERY_KIND: &str = "messaging/deliver";
pub const MESSAGING_REVISION: &str = "conduit.std/messaging-delivery@2";
pub const MESSAGING_PROFILE: &str = "std/messaging-deterministic-hosted@1";
pub const MESSAGING_ARTIFACT: &str = "conduit-std-host/messaging-deterministic@1";
pub const MESSAGING_HOST_OPERATION: &str = "conduit.host/messaging-deterministic@1";
pub const MESSAGING_DELIVERY_AUTHORITY: &str = "conduit.authority/messaging-deliver@1";

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

pub fn messaging_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            MESSAGING_MESSAGE_KIND,
            vec![],
            vec![
                port("message", &portable_message_type(), PortDirection::Output),
                port("request", &delivery_request_type(), PortDirection::Output),
            ],
        ),
        offer(
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

fn offer(kind: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> CapabilityOffer {
    let operation_target = if kind == MESSAGING_DELIVERY_KIND {
        delivery_request_type()
            .profile()
            .expect("reviewed delivery request profile")
            .value_kind()
            .clone()
    } else {
        kind_id(kind)
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(MESSAGING_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(MESSAGING_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(MESSAGING_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(MESSAGING_HOST_OPERATION),
            target_kind: Some(operation_target.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: (kind == MESSAGING_DELIVERY_KIND)
            .then(|| AuthorityRequirement {
                contract_id: AuthorityContractId::from(MESSAGING_DELIVERY_AUTHORITY),
                host_operation_contract_id: HostOperationContractId::from(MESSAGING_HOST_OPERATION),
                subject_kind: operation_target,
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
