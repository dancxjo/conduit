use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer,
};

pub const BODY_CHAT_PROMPT_STD_IMPLEMENTATION: &str = "std/body-chat-prompt@2";
pub const BODY_CHAT_PROMPT_STD_PROFILE: &str = "std/body-chat-prompt-kernel@1";
pub const BODY_CHAT_PROMPT_STD_ARTIFACT: &str = "conduit-std-host/body-chat-prompt@2";
pub const BODY_CHAT_MESSAGE_OPERATION: &str = "conduit.host/body-chat-message@1";
pub const BODY_CHAT_RESPONSE_OPERATION: &str = "conduit.host/body-chat-response@1";
pub const BODY_CHAT_CONTEXT_OPERATION: &str = "conduit.host/body-chat-context@1";
pub const BODY_CONVERSATION_CONTEXT_STD_IMPLEMENTATION: &str = "std/body-conversation-context@2";
pub const BODY_CONVERSATION_CONTEXT_STD_PROFILE: &str = "std/body-conversation-context-kernel@1";
pub const BODY_CONVERSATION_CONTEXT_STD_ARTIFACT: &str =
    "conduit-std-host/body-conversation-context@2";
pub const BODY_CONVERSATION_CONTEXT_OPERATION: &str = "conduit.host/body-conversation-context@2";

pub fn body_chat_prompt_std_offer() -> CapabilityOffer {
    let definition = conduit_chat::body_chat_prompt_definition();
    let operation = |id, input, output| HostOperationRequirement {
        contract_id: HostOperationContractId::from(id),
        target_kind: Some(definition.kind_id.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("body-chat-prompt"),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BODY_CHAT_PROMPT_STD_PROFILE),
            implementation_id: ImplementationId::from(BODY_CHAT_PROMPT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(BODY_CHAT_PROMPT_STD_ARTIFACT),
        },
        host_operations: vec![
            operation(
                BODY_CHAT_MESSAGE_OPERATION,
                conduit_chat::MAXIMUM_BODY_CHAT_MESSAGE_BYTES as u32,
                conduit_chat::MAXIMUM_BODY_CHAT_PROMPT_BYTES as u32,
            ),
            operation(
                BODY_CHAT_RESPONSE_OPERATION,
                conduit_chat::MAXIMUM_BODY_CHAT_MESSAGE_BYTES as u32,
                conduit_chat::MAXIMUM_BODY_CHAT_MESSAGE_BYTES as u32,
            ),
            operation(
                BODY_CHAT_CONTEXT_OPERATION,
                conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES as u32,
                0,
            ),
        ],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: conduit_chat::body_chat_prompt_limits(),
    }
}

pub fn body_conversation_context_std_offer() -> CapabilityOffer {
    let definition = conduit_chat::body_conversation_context_definition();
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("body-conversation-context"),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BODY_CONVERSATION_CONTEXT_STD_PROFILE),
            implementation_id: ImplementationId::from(BODY_CONVERSATION_CONTEXT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(BODY_CONVERSATION_CONTEXT_STD_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(BODY_CONVERSATION_CONTEXT_OPERATION),
            target_kind: Some(definition.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES as u32,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES as u32,
        },
    }
}
