use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, ImplementationId, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal,
};

pub const CHAT_PEER_KIND: &str = "chat/peer";
pub const CHAT_ROOM_KIND: &str = "chat/room";
pub const FLOW_FAN_KIND: &str = "flow/fan";
pub const FLOW_MERGE_KIND: &str = "flow/merge";
pub const CHAT_MESSAGE_KIND: &str = "ChatMessage";
pub const POOL_WEBCHAT_MAXIMUM_PEERS: u16 = 32;

pub fn pool_chat_capabilities() -> [CapabilityOffer; 4] {
    [
        offer(
            CHAT_PEER_KIND,
            vec![input("recv")],
            vec![output("send")],
            32,
        ),
        offer(CHAT_ROOM_KIND, vec![input("recv")], vec![output("send")], 1),
        offer(FLOW_FAN_KIND, vec![input("message")], Vec::new(), 1),
        offer(FLOW_MERGE_KIND, Vec::new(), vec![output("message")], 1),
    ]
}

fn offer(
    kind: &str,
    inputs: alloc::vec::Vec<PortDescriptor>,
    outputs: alloc::vec::Vec<PortDescriptor>,
    maximum_instances: u16,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: if kind == CHAT_PEER_KIND {
            Vec::new()
        } else {
            vec![FaceStartupParameter {
                name: "members".to_string(),
                value_type: "Pool".to_string(),
                has_default: false,
            }]
        },
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/pool-webchat/{kind}")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("conduit.{kind}@1")),
        execution_profile_id: ExecutionProfileId::from("conduit.chat/shared-pool-hosted@1"),
        implementation_id: ImplementationId::from(format!("std/pool-webchat/{kind}@1")),
        artifact_id: ArtifactId::from("conduit-std-host/pool-webchat@1"),
        inputs,
        outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: maximum_instances,
            max_queue_items: 32,
            max_queue_bytes: 8_192,
        },
    }
}

fn input(name: &str) -> PortDescriptor {
    port(name, PortDirection::Input)
}

fn output(name: &str) -> PortDescriptor {
    port(name, PortDirection::Output)
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(CHAT_MESSAGE_KIND),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_pool_chat_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, OperationSignature, StartupParameterSignature};

    for offer in pool_chat_capabilities().into_iter().skip(1) {
        startup.insert(OperationSignature {
            operation: offer.kind_id.as_str().to_string(),
            startup_parameters: vec![StartupParameterSignature {
                name: "members".to_string(),
                value_type: "Pool".to_string(),
                default: None,
            }],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
