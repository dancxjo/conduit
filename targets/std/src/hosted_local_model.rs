//! Explicit initialized local-model adapter below portable L0 semantics.

use conduit_core::PlannedGear;

mod ollama;
pub(crate) mod ollama_present;
mod ollama_stream;
pub use conduit_ai::{LocalModelKindProfile, MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES};
pub use ollama::{OllamaDiscovery, OllamaLocalModelAdapter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalModelAdapterTerminal {
    Produced,
    Truncated,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
    InvalidStructuredResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingChunkDisposition {
    Accepted,
    Backpressured,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalModelStreamStep {
    Chunk(conduit_ai::GeneratedTextChunk),
    Terminal(conduit_ai::GeneratedTextFlowEvidence),
}

pub trait HostedLocalModelAdapter: Send {
    fn offer(&self) -> &conduit_ai::LocalModelOffer;

    /// Refresh provider availability without changing the stable offer.
    fn current_pool_health(&self) -> conduit_core::PoolRealizationHealth {
        conduit_core::PoolRealizationHealth::Unavailable
    }

    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal;

    fn execute_stream(
        &mut self,
        _placement: &PlannedGear,
        _input: &[u8],
        _sink: &mut dyn FnMut(&conduit_ai::GeneratedTextChunk) -> StreamingChunkDisposition,
    ) -> conduit_ai::GeneratedTextFlowEvidence {
        conduit_ai::GeneratedTextFlowEvidence {
            chunks: 0,
            generated_bytes: 0,
            terminal: conduit_ai::GeneratedTextFlowTerminal::ProviderLost,
            retained_private_text: false,
        }
    }

    fn execute_stream_step(
        &mut self,
        _placement: &PlannedGear,
        _input: &[u8],
    ) -> LocalModelStreamStep {
        LocalModelStreamStep::Terminal(conduit_ai::GeneratedTextFlowEvidence {
            chunks: 0,
            generated_bytes: 0,
            terminal: conduit_ai::GeneratedTextFlowTerminal::ProviderLost,
            retained_private_text: false,
        })
    }

    fn cancel_stream(&mut self) {}
}

pub(crate) fn resource_offers(
    limits: &conduit_ai::LocalModelLimits,
) -> Vec<conduit_core::ResourceOffer> {
    vec![
        conduit_core::resource_offer(
            "std/local-model-memory",
            conduit_ai::LOCAL_MODEL_MEMORY_RESOURCE,
            limits.admitted_memory_mib,
        ),
        conduit_core::compute_resource_offer(
            "std/local-model-compute",
            conduit_ai::LOCAL_MODEL_COMPUTE_RESOURCE,
            limits.compute.maximum_lanes,
            conduit_core::ComputePoolContract {
                service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
                architecture_base_id: conduit_core::ArchitectureBaseId::from(
                    "std/hosted-compute@1",
                ),
                architecture_base_kind: conduit_core::ArchitectureBaseKind::HostedOs,
                topology_groups: Vec::new(),
            },
        ),
        conduit_core::resource_offer(
            "std/local-model-inference-slots",
            conduit_ai::LOCAL_MODEL_INFERENCE_SLOT_RESOURCE,
            u32::from(limits.maximum_in_flight),
        ),
        conduit_core::resource_offer(
            "std/local-model-queue-items",
            conduit_ai::LOCAL_MODEL_QUEUE_ITEM_RESOURCE,
            u32::from(limits.maximum_queue_items),
        ),
        conduit_core::resource_offer(
            "std/local-model-queue-kib",
            conduit_ai::LOCAL_MODEL_QUEUE_KIB_RESOURCE,
            limits.maximum_queue_bytes.div_ceil(1024),
        ),
    ]
}

/// Project the canonical one-prompt/one-text front onto the same initialized
/// local-model realization. This is an additional exact back, not an alias:
/// it keeps the portable `ai/generate-text` meaning while retaining the
/// model-specific artifact and finite resource obligations.
pub(crate) fn generate_text_capability_offer(
    offer: &conduit_ai::LocalModelOffer,
) -> Result<conduit_core::CapabilityOffer, String> {
    let generate = offer
        .capability_offers()
        .map_err(|error| format!("local-model capabilities: {error:?}"))?
        .into_iter()
        .find(|candidate| candidate.kind_id.as_str() == conduit_ai::LLM_GENERATE_KIND)
        .ok_or_else(|| "local-model offer does not include generation".to_string())?;
    let contract = conduit_ai::generate_text_contract();
    let maximum_input_bytes = u32::try_from(offer.limits.work.maximum_input_bytes)
        .map_err(|_| "local-model input bound exceeds the canonical Host operation".to_string())?;
    let maximum_output_bytes = u32::try_from(offer.limits.work.maximum_output_bytes)
        .map_err(|_| "local-model output bound exceeds the canonical Host operation".to_string())?;
    Ok(conduit_core::CapabilityOffer {
        startup_parameters: [
            "maximum-input-bytes",
            "maximum-context-tokens",
            "maximum-output-tokens",
            "temperature-milli",
        ]
        .into_iter()
        .map(|name| conduit_core::FaceStartupParameter {
            name: name.into(),
            value_type: "Count".into(),
            has_default: true,
        })
        .collect(),
        shorthand: None,
        capability_id: conduit_core::CapabilityId::from("local-model/ai/generate-text"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: conduit_core::ExecutionProfileId::from(
                "conduit.ai/generate-text-hosted@1",
            ),
            implementation_id: generate.implementation.implementation_id,
            artifact_id: generate.implementation.artifact_id,
        },
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                conduit_ai::GENERATE_TEXT_HOST_OPERATION,
            ),
            target_kind: Some(conduit_core::KindId::from(conduit_ai::GENERATE_TEXT_KIND)),
            maximum_in_flight: offer.limits.maximum_in_flight,
            maximum_input_bytes,
            maximum_output_bytes,
        }],
        resource_requirements: generate.resource_requirements,
        authority_requirements: Vec::new(),
        limits: generate.limits,
    })
}

#[cfg(test)]
mod tests;
