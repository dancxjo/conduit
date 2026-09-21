use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId,
};

pub const BODY_CHAT_PROMPT_STD_IMPLEMENTATION: &str = "std/body-chat-prompt@3";
pub const BODY_CHAT_PROMPT_STD_PROFILE: &str = "std/body-chat-prompt-kernel@1";
pub const BODY_CHAT_PROMPT_STD_ARTIFACT: &str = "conduit-std-host/body-chat-prompt@3";
pub const BODY_CHAT_MESSAGE_OPERATION: &str = "conduit.host/body-chat-message@1";
pub const BODY_CHAT_RESPONSE_OPERATION: &str = "conduit.host/body-chat-response@1";
pub const BODY_CHAT_CONTEXT_OPERATION: &str = "conduit.host/body-chat-context@1";
pub const BODY_CONVERSATION_CONTEXT_STD_IMPLEMENTATION: &str = "std/body-conversation-context@2";
pub const BODY_CONVERSATION_CONTEXT_STD_PROFILE: &str = "std/body-conversation-context-kernel@1";
pub const BODY_CONVERSATION_CONTEXT_STD_ARTIFACT: &str =
    "conduit-std-host/body-conversation-context@2";
pub const BODY_CONVERSATION_CONTEXT_OPERATION: &str = "conduit.host/body-conversation-context@2";

pub fn body_chat_prompt_std_offer() -> CapabilityOffer {
    let contract = conduit_chat::body_chat_prompt_semantic_contract();
    let target_kind = contract.kind_id.clone();
    let operation = |id, input, output| HostCallRequirement {
        contract_id: HostCallContractId::from(id),
        target_kind: Some(target_kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    };
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("body-chat-prompt"),
            execution_profile_id: ExecutionProfileId::from(BODY_CHAT_PROMPT_STD_PROFILE),
            implementation_id: ImplementationId::from(BODY_CHAT_PROMPT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(BODY_CHAT_PROMPT_STD_ARTIFACT),
            host_calls: vec![
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
        },
    )
    .build()
}

pub fn body_conversation_context_std_offer() -> CapabilityOffer {
    let contract = conduit_chat::body_conversation_context_semantic_contract();
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("body-conversation-context"),
            execution_profile_id: ExecutionProfileId::from(BODY_CONVERSATION_CONTEXT_STD_PROFILE),
            implementation_id: ImplementationId::from(BODY_CONVERSATION_CONTEXT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(BODY_CONVERSATION_CONTEXT_STD_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(BODY_CONVERSATION_CONTEXT_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: 0,
                maximum_output_bytes: conduit_chat::MAXIMUM_BODY_CHAT_CONTEXT_BYTES as u32,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_body_chat_semantics_and_bounded_operations() {
        let prompt = body_chat_prompt_std_offer();
        let prompt_contract = conduit_chat::body_chat_prompt_semantic_contract();
        assert_eq!(
            prompt.kind_contract_revision,
            prompt_contract.kind_contract_revision
        );
        assert_eq!(prompt.inputs, prompt_contract.inputs);
        assert_eq!(prompt.outputs, prompt_contract.outputs);
        assert_eq!(prompt.limits, prompt_contract.limits);
        assert_eq!(prompt.host_calls.len(), 3);

        let context = body_conversation_context_std_offer();
        let context_contract = conduit_chat::body_conversation_context_semantic_contract();
        assert_eq!(context.outputs, context_contract.outputs);
        assert_eq!(context.limits, context_contract.limits);
        assert_eq!(context.host_calls.len(), 1);
        assert!(prompt.authority_requirements.is_empty());
        assert!(context.authority_requirements.is_empty());
    }
}
