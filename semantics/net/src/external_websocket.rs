use alloc::string::ToString;
use alloc::vec;

use conduit_core::{
    kind_id, port_id, resource_offer, resource_requirement, ArtifactId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, ResourceOffer,
};

/// Authored external WebSocket semantics. This is not a Conduit session line.
pub const EXTERNAL_WEBSOCKET_CLIENT_KIND: &str = "net/websocket";
pub const EXTERNAL_WEBSOCKET_LISTENER_KIND: &str = "net/websocket/listen";
pub const EXTERNAL_WEBSOCKET_CLIENT_REVISION: &str = "conduit.net/websocket-client@1";
pub const EXTERNAL_WEBSOCKET_LISTENER_REVISION: &str = "conduit.net/websocket-listener@1";
pub const EXTERNAL_WEBSOCKET_CLIENT_PROFILE: &str = "conduit.net/websocket-client-hosted@1";
pub const EXTERNAL_WEBSOCKET_LISTENER_PROFILE: &str = "conduit.net/websocket-listener-hosted@1";
pub const EXTERNAL_WEBSOCKET_CLIENT_OPEN_HOST_OPERATION: &str =
    "conduit.host/external-websocket-client-open@1";
pub const EXTERNAL_WEBSOCKET_CLIENT_SEND_HOST_OPERATION: &str =
    "conduit.host/external-websocket-client-send@1";
pub const EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION: &str =
    "conduit.host/external-websocket-client-receive@1";
pub const EXTERNAL_WEBSOCKET_CLIENT_CLOSE_HOST_OPERATION: &str =
    "conduit.host/external-websocket-client-close@1";
pub const EXTERNAL_WEBSOCKET_LISTENER_ACCEPT_HOST_OPERATION: &str =
    "conduit.host/external-websocket-listener-accept@1";
pub const EXTERNAL_WEBSOCKET_LISTENER_RECEIVE_HOST_OPERATION: &str =
    "conduit.host/external-websocket-listener-receive@1";
pub const EXTERNAL_WEBSOCKET_LISTENER_SEND_HOST_OPERATION: &str =
    "conduit.host/external-websocket-listener-send@1";
pub const EXTERNAL_WEBSOCKET_CLIENT_RESOURCE: &str =
    "conduit.resource/network/external-websocket-client@1";
pub const EXTERNAL_WEBSOCKET_LISTENER_RESOURCE: &str =
    "conduit.resource/network/external-websocket-listener@1";

pub const URL_VALUE_KIND: &str = "value/net-url@1";
pub const NET_ADDRESS_VALUE_KIND: &str = "value/net-address@1";
/// One complete RFC 6455 binary message. Bases must reject text frames,
/// fragmented values beyond the admitted message bound, and malformed frames.
pub const WEBSOCKET_MESSAGE_VALUE_KIND: &str = "WebSocketMessage";
pub const BOOLEAN_VALUE_KIND: &str = "Boolean";
pub const PEER_EVENT_VALUE_KIND: &str = "NetPeerEvent";
pub const PEER_MESSAGE_VALUE_KIND: &str = "NetPeerMessage";

pub const MAXIMUM_EXTERNAL_WEBSOCKET_PEERS: u16 = 2;
pub const MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES: u32 = 256;
pub const MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES: u32 =
    MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES + 2;
pub const MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_ITEMS: u16 = 8;
pub const MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_BYTES: u32 =
    MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES * MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_ITEMS as u32;
pub const MAXIMUM_EXTERNAL_WEBSOCKET_HISTORY_ITEMS: u16 = 16;

pub fn external_websocket_client_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![startup("url", "Url")],
        shorthand: None,
        capability_id,
        kind_id: kind_id(EXTERNAL_WEBSOCKET_CLIENT_KIND),
        kind_contract_revision: KindContractRevision::from(EXTERNAL_WEBSOCKET_CLIENT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(EXTERNAL_WEBSOCKET_CLIENT_PROFILE),
            implementation_id,
            artifact_id,
        },
        inputs: vec![port(
            "send",
            WEBSOCKET_MESSAGE_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![
            port(
                "recv",
                WEBSOCKET_MESSAGE_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "live",
                BOOLEAN_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Current,
            ),
        ],
        host_operations: vec![
            host_operation(EXTERNAL_WEBSOCKET_CLIENT_CLOSE_HOST_OPERATION, 1, 0),
            host_operation(EXTERNAL_WEBSOCKET_CLIENT_OPEN_HOST_OPERATION, 256, 1),
            host_operation(
                EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION,
                MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
                MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
            ),
            host_operation(
                EXTERNAL_WEBSOCKET_CLIENT_SEND_HOST_OPERATION,
                MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
                MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
            ),
        ],
        resource_requirements: vec![resource_requirement(EXTERNAL_WEBSOCKET_CLIENT_RESOURCE, 1)],
        authority_requirements: vec![],
        limits: limits(1),
    }
}

pub fn external_websocket_listener_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![startup("bind", "NetAddress")],
        shorthand: None,
        capability_id,
        kind_id: kind_id(EXTERNAL_WEBSOCKET_LISTENER_KIND),
        kind_contract_revision: KindContractRevision::from(EXTERNAL_WEBSOCKET_LISTENER_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(EXTERNAL_WEBSOCKET_LISTENER_PROFILE),
            implementation_id,
            artifact_id,
        },
        inputs: vec![port(
            "send",
            PEER_MESSAGE_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![
            port(
                "peer",
                PEER_EVENT_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "recv",
                PEER_MESSAGE_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "live",
                BOOLEAN_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Current,
            ),
        ],
        host_operations: vec![
            host_operation(EXTERNAL_WEBSOCKET_LISTENER_ACCEPT_HOST_OPERATION, 64, 8),
            host_operation(
                EXTERNAL_WEBSOCKET_LISTENER_RECEIVE_HOST_OPERATION,
                MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
                MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
            ),
            host_operation(
                EXTERNAL_WEBSOCKET_LISTENER_SEND_HOST_OPERATION,
                MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
                MAXIMUM_EXTERNAL_WEBSOCKET_PEER_MESSAGE_BYTES,
            ),
        ],
        resource_requirements: vec![resource_requirement(
            EXTERNAL_WEBSOCKET_LISTENER_RESOURCE,
            1,
        )],
        authority_requirements: vec![],
        limits: limits(MAXIMUM_EXTERNAL_WEBSOCKET_PEERS),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWebSocketFamily {
    pub resource: ResourceOffer,
    pub capability: CapabilityOffer,
}

pub fn browser_external_websocket_family() -> ExternalWebSocketFamily {
    ExternalWebSocketFamily {
        resource: resource_offer(
            "browser/external-websocket-client-0",
            EXTERNAL_WEBSOCKET_CLIENT_RESOURCE,
            1,
        ),
        capability: external_websocket_client_offer(
            CapabilityId::from("browser/external-websocket-client"),
            ImplementationId::from("browser/native-external-websocket-client@1"),
            ArtifactId::from("conduit-browser-runtime/external-websocket-client@1"),
        ),
    }
}

pub fn std_external_websocket_family() -> ExternalWebSocketFamily {
    ExternalWebSocketFamily {
        resource: resource_offer(
            "std/external-websocket-listener-0",
            EXTERNAL_WEBSOCKET_LISTENER_RESOURCE,
            1,
        ),
        capability: external_websocket_listener_offer(
            CapabilityId::from("std/external-websocket-listener"),
            ImplementationId::from("std/native-external-websocket-listener@1"),
            ArtifactId::from("conduit-std-host/external-websocket-listener@1"),
        ),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_external_websocket_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_core::ConfigurationValue;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    for offer in [
        browser_external_websocket_family().capability,
        std_external_websocket_family().capability,
    ] {
        startup.insert(KindSignature {
            kind: offer.kind_id.as_str().to_string(),
            startup_parameters: offer
                .startup_parameters
                .iter()
                .map(|parameter| StartupParameterSignature {
                    name: parameter.name.clone(),
                    value_type: parameter.value_type.clone(),
                    default: None,
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: offer
                    .startup_parameters
                    .into_iter()
                    .map(|parameter| ConfigurationField {
                        key: parameter.name,
                        default_value: ConfigurationValue::Text(alloc::string::String::new()),
                        validation: ConfigurationRule::Any,
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn startup(name: &str, value_type: &str) -> FaceStartupParameter {
    FaceStartupParameter {
        name: name.to_string(),
        value_type: value_type.to_string(),
        has_default: false,
    }
}

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
    }
}

fn host_operation(contract: &str, input: u32, output: u32) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }
}

fn limits(active: u16) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: active,
        max_queue_items: MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_ITEMS,
        max_queue_bytes: MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_BYTES,
    }
}
