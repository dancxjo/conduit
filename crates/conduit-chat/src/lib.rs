#![no_std]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, resource_offer, resource_requirement, ArtifactId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, ResourceOffer,
};

pub const WEB_TEXT_INPUT_KIND: &str = "web/text-input";
pub const WEB_LIST_KIND: &str = "web/list";
pub const WEB_TEXT_INPUT_REVISION: &str = "conduit.web/text-input@1";
pub const WEB_LIST_REVISION: &str = "conduit.web/list@1";
pub const WEB_TEXT_INPUT_IMPLEMENTATION: &str = "browser/native-text-input@1";
pub const WEB_LIST_IMPLEMENTATION: &str = "browser/native-bounded-list@1";
pub const WEB_TEXT_INPUT_HOST_OPERATION: &str = "conduit.host/web-text-input@1";
pub const WEB_LIST_HOST_OPERATION: &str = "conduit.host/web-list-append@1";
pub const WEB_CHAT_RESOURCE: &str = "conduit.resource/presentation/web-chat@1";
pub const MAXIMUM_CHAT_HISTORY_ITEMS: u16 = 16;
pub const MAXIMUM_CHAT_INPUT_ITEMS: u16 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserChatFamily {
    pub resource: ResourceOffer,
    pub capabilities: [CapabilityOffer; 2],
}

pub fn browser_chat_family() -> BrowserChatFamily {
    BrowserChatFamily {
        resource: resource_offer("browser/web-chat-0", WEB_CHAT_RESOURCE, 2),
        capabilities: [text_input_offer(), list_offer()],
    }
}

pub fn text_input_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "label".to_string(),
            value_type: "Text".to_string(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("browser/web-text-input"),
        kind_id: kind_id(WEB_TEXT_INPUT_KIND),
        kind_contract_revision: KindContractRevision::from(WEB_TEXT_INPUT_REVISION),
        execution_profile_id: ExecutionProfileId::from("conduit.web/text-input-hosted@1"),
        implementation_id: ImplementationId::from(WEB_TEXT_INPUT_IMPLEMENTATION),
        artifact_id: ArtifactId::from("conduit-browser-runtime/web-text-input@1"),
        inputs: vec![],
        outputs: vec![port("message", PortDirection::Output)],
        host_operations: vec![host_operation(WEB_TEXT_INPUT_HOST_OPERATION)],
        resource_requirements: vec![resource_requirement(WEB_CHAT_RESOURCE, 1)],
        authority_requirements: vec![],
        limits: limits(MAXIMUM_CHAT_INPUT_ITEMS),
    }
}

pub fn list_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "maximum-items".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("browser/web-list"),
        kind_id: kind_id(WEB_LIST_KIND),
        kind_contract_revision: KindContractRevision::from(WEB_LIST_REVISION),
        execution_profile_id: ExecutionProfileId::from("conduit.web/list-hosted@1"),
        implementation_id: ImplementationId::from(WEB_LIST_IMPLEMENTATION),
        artifact_id: ArtifactId::from("conduit-browser-runtime/web-list@1"),
        inputs: vec![port("message", PortDirection::Input)],
        outputs: vec![],
        host_operations: vec![host_operation(WEB_LIST_HOST_OPERATION)],
        resource_requirements: vec![resource_requirement(WEB_CHAT_RESOURCE, 1)],
        authority_requirements: vec![],
        limits: limits(MAXIMUM_CHAT_HISTORY_ITEMS),
    }
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(conduit_net::WEBSOCKET_MESSAGE_VALUE_KIND),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

fn host_operation(contract: &str) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
        maximum_output_bytes: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
    }
}

fn limits(items: u16) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: items,
        max_queue_bytes: conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES * u32::from(items),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_browser_chat_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_core::ConfigurationValue;
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, OperationSignature,
        StartupParameterSignature,
    };

    for offer in browser_chat_family().capabilities {
        startup.insert(OperationSignature {
            operation: offer.kind_id.as_str().to_string(),
            startup_parameters: offer
                .startup_parameters
                .iter()
                .map(|parameter| StartupParameterSignature {
                    name: parameter.name.clone(),
                    value_type: parameter.value_type.clone(),
                    default: match parameter.name.as_str() {
                        "label" => Some("\"Message\"".to_string()),
                        "maximum-items" => Some("16".to_string()),
                        _ => None,
                    },
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
                        key: parameter.name.clone(),
                        default_value: if parameter.name == "maximum-items" {
                            ConfigurationValue::U64(u64::from(MAXIMUM_CHAT_HISTORY_ITEMS))
                        } else {
                            ConfigurationValue::Text("Message".to_string())
                        },
                        validation: ConfigurationRule::Any,
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
