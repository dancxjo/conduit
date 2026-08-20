//! Portable chat-state/submit contracts and browser realization offers.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, resource_offer, resource_requirement, ArtifactId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal, ResourceOffer,
};
use conduit_presentation::{
    interaction_offer, presentation_tee_offer, renderer_offer, InteractionRealizationOffer,
    RendererRealizationOffer, INTERACTION_KIND, MAX_PRESENTATION_INTERACTION_BYTES,
    MAX_PRESENTATION_TOTAL_BYTES, PRESENTATION_TEE_KIND, PRESENTATION_VALUE_KIND, RENDERER_KIND,
};

use crate::{CHAT_SEND_ACTION, MAXIMUM_CHAT_HISTORY_ITEMS, MAXIMUM_CHAT_MESSAGE_BYTES};

pub const CHAT_STATE_KIND: &str = "chat/state";
pub const CHAT_SUBMIT_KIND: &str = "chat/submit";
pub const CHAT_STATE_REVISION: &str = "conduit.chat/state@1";
pub const CHAT_SUBMIT_REVISION: &str = "conduit.chat/submit@1";
pub const CHAT_STATE_MESSAGE_HOST_OPERATION: &str = "conduit.chat/state-message@1";
pub const CHAT_STATE_CONNECTION_HOST_OPERATION: &str = "conduit.chat/state-connection@1";
pub const CHAT_SUBMIT_HOST_OPERATION: &str = "conduit.chat/submit@1";
pub const BROWSER_RENDER_HOST_OPERATION: &str = "conduit.browser/present@1";
pub const BROWSER_INTERACTION_HOST_OPERATION: &str = "conduit.browser/interaction@1";
pub const BROWSER_DOCUMENT_RESOURCE: &str = "conduit.resource/browser-document@1";
pub const BROWSER_INPUT_RESOURCE: &str = "conduit.resource/browser-human-input@1";

pub const CHAT_CONFIGURATION_FIELDS: [(&str, &str); 7] = [
    ("title", "Text"),
    ("history-label", "Text"),
    ("input-label", "Text"),
    ("submit-label", "Text"),
    ("status-label", "Text"),
    ("maximum-history-items", "Count"),
    ("maximum-message-bytes", "Count"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserChatFamily {
    pub resources: [ResourceOffer; 2],
    pub capabilities: Vec<CapabilityOffer>,
}

pub fn browser_chat_family() -> BrowserChatFamily {
    BrowserChatFamily {
        resources: [
            resource_offer("browser/document-0", BROWSER_DOCUMENT_RESOURCE, 1),
            resource_offer("browser/human-input-0", BROWSER_INPUT_RESOURCE, 1),
        ],
        capabilities: vec![
            chat_state_offer(),
            presentation_tee_offer(
                CapabilityId::from("browser/presentation-tee"),
                ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from(
                        "conduit.presentation/tee-kernel@1",
                    ),
                    implementation_id: ImplementationId::from("presentation/kernel-tee@1"),
                    artifact_id: ArtifactId::from("conduit-browser-runtime/presentation-tee@1"),
                },
                limits(8, MAX_PRESENTATION_TOTAL_BYTES as u32 * 8),
            ),
            renderer_offer(RendererRealizationOffer {
                capability_id: CapabilityId::from("browser/presentation-renderer"),
                execution_profile_id: ExecutionProfileId::from(
                    "conduit.presentation/browser-renderer@1",
                ),
                implementation_id: ImplementationId::from("presentation/browser-semantic-dom@1"),
                artifact_id: ArtifactId::from("conduit-browser-runtime/semantic-dom@1"),
                host_operation: host_operation(
                    BROWSER_RENDER_HOST_OPERATION,
                    MAX_PRESENTATION_TOTAL_BYTES as u32,
                    16 * 1024,
                ),
                resource_requirement: resource_requirement(BROWSER_DOCUMENT_RESOURCE, 1),
                limits: limits(8, MAX_PRESENTATION_TOTAL_BYTES as u32 * 8),
            }),
            interaction_offer(InteractionRealizationOffer {
                capability_id: CapabilityId::from("browser/presentation-interaction"),
                execution_profile_id: ExecutionProfileId::from(
                    "conduit.presentation/browser-interaction@1",
                ),
                implementation_id: ImplementationId::from("presentation/browser-human-input@1"),
                artifact_id: ArtifactId::from("conduit-browser-runtime/human-input@1"),
                host_operation: HostOperationRequirement {
                    contract_id: HostOperationContractId::from(BROWSER_INTERACTION_HOST_OPERATION),
                    target_kind: Some(kind_id(
                        conduit_presentation::PRESENTATION_INTERACTION_VALUE_KIND,
                    )),
                    maximum_in_flight: 1,
                    maximum_input_bytes: 0,
                    maximum_output_bytes: MAX_PRESENTATION_INTERACTION_BYTES as u32,
                },
                resource_requirement: resource_requirement(BROWSER_INPUT_RESOURCE, 1),
                limits: limits(
                    conduit_presentation::MAX_QUEUED_PRESENTATION_INTERACTIONS as u16,
                    MAX_PRESENTATION_INTERACTION_BYTES as u32
                        * conduit_presentation::MAX_QUEUED_PRESENTATION_INTERACTIONS as u32,
                ),
            }),
            chat_submit_offer(),
        ],
    }
}

pub fn chat_state_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: CHAT_CONFIGURATION_FIELDS
            .iter()
            .map(|(name, value_type)| FaceStartupParameter {
                name: (*name).into(),
                value_type: (*value_type).into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from("browser/chat-state"),
        kind_id: kind_id(CHAT_STATE_KIND),
        kind_contract_revision: KindContractRevision::from(CHAT_STATE_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit.chat/state-kernel@1"),
            implementation_id: ImplementationId::from("chat/portable-state@1"),
            artifact_id: ArtifactId::from("conduit-browser-runtime/chat-state@1"),
        },
        inputs: chat_state_inputs(),
        outputs: vec![port(
            "presentation",
            PRESENTATION_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Value,
        )],
        host_operations: vec![
            host_operation(
                CHAT_STATE_CONNECTION_HOST_OPERATION,
                1,
                MAX_PRESENTATION_TOTAL_BYTES as u32,
            ),
            host_operation(
                CHAT_STATE_MESSAGE_HOST_OPERATION,
                MAXIMUM_CHAT_MESSAGE_BYTES,
                MAX_PRESENTATION_TOTAL_BYTES as u32,
            ),
        ],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: limits(
            MAXIMUM_CHAT_HISTORY_ITEMS as u16,
            MAX_PRESENTATION_TOTAL_BYTES as u32 * 2,
        ),
    }
}

pub fn chat_submit_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![
            FaceStartupParameter {
                name: "action".into(),
                value_type: "Text".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "maximum-message-bytes".into(),
                value_type: "Count".into(),
                has_default: true,
            },
        ],
        shorthand: None,
        capability_id: CapabilityId::from("browser/chat-submit"),
        kind_id: kind_id(CHAT_SUBMIT_KIND),
        kind_contract_revision: KindContractRevision::from(CHAT_SUBMIT_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit.chat/submit-kernel@1"),
            implementation_id: ImplementationId::from("chat/typed-submit@1"),
            artifact_id: ArtifactId::from("conduit-browser-runtime/chat-submit@1"),
        },
        inputs: vec![port(
            "interaction",
            conduit_presentation::PRESENTATION_INTERACTION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "message",
            conduit_net::WEBSOCKET_MESSAGE_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        host_operations: vec![host_operation(
            CHAT_SUBMIT_HOST_OPERATION,
            MAX_PRESENTATION_INTERACTION_BYTES as u32,
            MAXIMUM_CHAT_MESSAGE_BYTES,
        )],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: limits(8, MAX_PRESENTATION_INTERACTION_BYTES as u32 * 8),
    }
}

fn chat_state_inputs() -> Vec<PortDescriptor> {
    vec![
        port(
            "message",
            conduit_net::WEBSOCKET_MESSAGE_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        ),
        port(
            "live",
            conduit_net::BOOLEAN_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Current,
        ),
    ]
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

fn limits(items: u16, bytes: u32) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: items,
        max_queue_bytes: bytes,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_browser_chat_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature, StartupParameterSignature};

    for kind in [INTERACTION_KIND, PRESENTATION_TEE_KIND, RENDERER_KIND] {
        startup.insert(KindSignature {
            kind: kind.into(),
            startup_parameters: Vec::new(),
        })?;
    }
    profile
        .insert(conduit_presentation::interaction_kind_definition())
        .map_err(|error| error.to_string())?;
    profile
        .insert(conduit_presentation::presentation_tee_kind_definition())
        .map_err(|error| error.to_string())?;
    profile
        .insert(conduit_presentation::renderer_kind_definition())
        .map_err(|error| error.to_string())?;

    startup.insert(KindSignature {
        kind: CHAT_STATE_KIND.into(),
        startup_parameters: CHAT_CONFIGURATION_FIELDS
            .iter()
            .map(|(name, value_type)| StartupParameterSignature {
                name: (*name).into(),
                value_type: (*value_type).into(),
                default: Some(default_source(name)),
            })
            .collect(),
    })?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(CHAT_STATE_KIND),
            kind_contract_revision: KindContractRevision::from(CHAT_STATE_REVISION),
            inputs: chat_state_inputs(),
            outputs: chat_state_offer().outputs,
            configuration: CHAT_CONFIGURATION_FIELDS
                .iter()
                .map(|(name, value_type)| configuration(name, value_type))
                .collect(),
        })
        .map_err(|error| error.to_string())?;

    startup.insert(KindSignature {
        kind: CHAT_SUBMIT_KIND.into(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "action".into(),
                value_type: "Text".into(),
                default: Some(format_text(CHAT_SEND_ACTION)),
            },
            StartupParameterSignature {
                name: "maximum-message-bytes".into(),
                value_type: "Count".into(),
                default: Some(MAXIMUM_CHAT_MESSAGE_BYTES.to_string()),
            },
        ],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(CHAT_SUBMIT_KIND),
            kind_contract_revision: KindContractRevision::from(CHAT_SUBMIT_REVISION),
            inputs: chat_submit_offer().inputs,
            outputs: chat_submit_offer().outputs,
            configuration: vec![
                configuration("action", "Text"),
                configuration("maximum-message-bytes", "Count"),
            ],
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(feature = "form-catalog")]
fn configuration(name: &str, value_type: &str) -> conduit_form::ConfigurationField {
    use conduit_core::ConfigurationValue;
    use conduit_form::{ConfigurationField, ConfigurationRule};
    if value_type == "Count" {
        let maximum = if name == "maximum-history-items" {
            MAXIMUM_CHAT_HISTORY_ITEMS as u64
        } else {
            MAXIMUM_CHAT_MESSAGE_BYTES as u64
        };
        ConfigurationField {
            key: name.into(),
            default_value: ConfigurationValue::U64(maximum),
            validation: ConfigurationRule::U64Range {
                minimum: 1,
                maximum,
            },
        }
    } else {
        let value = default_text(name);
        ConfigurationField {
            key: name.into(),
            default_value: ConfigurationValue::Text(value.into()),
            validation: ConfigurationRule::TextBytes { maximum: 256 },
        }
    }
}

#[cfg(feature = "form-catalog")]
fn default_source(name: &str) -> String {
    if matches!(name, "maximum-history-items" | "maximum-message-bytes") {
        if name == "maximum-history-items" {
            MAXIMUM_CHAT_HISTORY_ITEMS.to_string()
        } else {
            MAXIMUM_CHAT_MESSAGE_BYTES.to_string()
        }
    } else {
        format_text(default_text(name))
    }
}

#[cfg(feature = "form-catalog")]
fn default_text(name: &str) -> &'static str {
    match name {
        "title" => "Conduit Webchat",
        "history-label" => "Chat history",
        "input-label" => "Message",
        "submit-label" => "Send",
        "status-label" => "Connection",
        "action" => CHAT_SEND_ACTION,
        _ => "Chat",
    }
}

#[cfg(feature = "form-catalog")]
fn format_text(value: &str) -> String {
    alloc::format!("\"{value}\"")
}
