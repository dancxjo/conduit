//! Explicit initialized local-model adapter below portable L0 semantics.

use conduit_core::PlannedGear;

mod ollama;
mod ollama_stream;
pub use conduit_ai::LocalModelKindProfile;
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

#[cfg(test)]
mod tests;
