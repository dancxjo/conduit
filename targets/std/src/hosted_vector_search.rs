//! Production adapters from the portable vector-search Host operation to exact and HNSW indexes.

use conduit_ai::{
    exact_vector_search, exact_vector_search_offer, ExactVectorSearchCandidate,
    ExactVectorSearchRefusal, SimilarityQuery, VectorIndexHandle, VectorIndexQueryAdmission,
    VectorIndexState, VectorSearchExecutionProofClass, VectorSearchValue,
    MAXIMUM_VECTOR_SEARCH_INPUT_BYTES, MAXIMUM_VECTOR_SEARCH_OUTPUT_BYTES,
    VECTOR_SEARCH_RESOURCE_CLASS,
};
use conduit_core::{CapabilityOffer, PlannedGear, ResourceBinding, ResourceOffer};

use crate::hosted_vector_index::{
    hosted_hnsw_vector_search_offer, HostedHnswRefusal, HostedHnswVectorIndex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedVectorSearchTerminal {
    Produced,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
    QueueFull,
    MalformedInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorSearchSessionRefusal {
    QueueFull,
    CancelledLateCompletion,
    StaleCompletion,
    SequenceExhausted,
}

#[derive(Debug, Default)]
pub struct VectorSearchSession {
    next_request: u64,
    active_request: Option<u64>,
    cancelled_request: Option<u64>,
}

impl VectorSearchSession {
    pub fn begin(&mut self) -> Result<u64, VectorSearchSessionRefusal> {
        if self.active_request.is_some() {
            return Err(VectorSearchSessionRefusal::QueueFull);
        }
        let request = self.next_request;
        self.next_request = request
            .checked_add(1)
            .ok_or(VectorSearchSessionRefusal::SequenceExhausted)?;
        self.active_request = Some(request);
        Ok(request)
    }

    pub fn complete(&mut self, request: u64) -> Result<(), VectorSearchSessionRefusal> {
        if self.cancelled_request == Some(request) {
            return Err(VectorSearchSessionRefusal::CancelledLateCompletion);
        }
        if self.active_request != Some(request) {
            return Err(VectorSearchSessionRefusal::StaleCompletion);
        }
        self.active_request = None;
        Ok(())
    }

    pub fn cancel(&mut self) -> Option<u64> {
        let request = self.active_request.take()?;
        self.cancelled_request = Some(request);
        Some(request)
    }
}

pub trait HostedVectorSearchAdapter: Send {
    fn capability_offer(&self) -> &CapabilityOffer;
    fn resource_offer(&self) -> &ResourceOffer;
    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> HostedVectorSearchTerminal;
    fn cancel(&mut self);
}

pub struct ExactVectorSearchAdapter {
    capability: CapabilityOffer,
    resource: ResourceOffer,
    state: VectorIndexState,
    handle: VectorIndexHandle,
    candidates: Vec<ExactVectorSearchCandidate<String>>,
    earliest_history_complete: bool,
    session: VectorSearchSession,
}

impl ExactVectorSearchAdapter {
    pub fn new(
        process_identity: &str,
        state: VectorIndexState,
        handle: VectorIndexHandle,
        candidates: Vec<ExactVectorSearchCandidate<String>>,
        earliest_history_complete: bool,
    ) -> Result<Self, String> {
        let capability = exact_vector_search_offer(process_identity)
            .map_err(|error| format!("exact vector-search offer: {error:?}"))?;
        let resource = state
            .contract
            .planning_offer()
            .map_err(|error| format!("exact vector-search resource: {error:?}"))?;
        Ok(Self {
            capability,
            resource,
            state,
            handle,
            candidates,
            earliest_history_complete,
            session: VectorSearchSession::default(),
        })
    }

    fn execute_request(
        &self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> HostedVectorSearchTerminal {
        let Ok((query, admission, binding)) = decode_request(&self.capability, placement, input)
        else {
            return HostedVectorSearchTerminal::MalformedInput;
        };
        match exact_vector_search(
            &self.state,
            &self.handle,
            &query,
            &self.candidates,
            admission,
            binding,
            self.earliest_history_complete,
        ) {
            Ok(result) => encode_output(
                VectorSearchValue {
                    proof_class: VectorSearchExecutionProofClass::DeterministicExact,
                    index_generation: result.index_generation,
                    admitted_work_units: result.admitted_work_units,
                    candidate_count: result.candidate_count,
                    hits: result.hits,
                },
                output,
            ),
            Err(ExactVectorSearchRefusal::Resource(_)) => HostedVectorSearchTerminal::Refused,
            Err(_) => HostedVectorSearchTerminal::Refused,
        }
    }
}

impl HostedVectorSearchAdapter for ExactVectorSearchAdapter {
    fn capability_offer(&self) -> &CapabilityOffer {
        &self.capability
    }

    fn resource_offer(&self) -> &ResourceOffer {
        &self.resource
    }

    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> HostedVectorSearchTerminal {
        let Ok(request) = self.session.begin() else {
            return HostedVectorSearchTerminal::QueueFull;
        };
        let result = self.execute_request(placement, input, output);
        if self.session.complete(request).is_err() {
            return HostedVectorSearchTerminal::Cancelled;
        }
        result
    }

    fn cancel(&mut self) {
        self.session.cancel();
    }
}

pub struct HnswVectorSearchAdapter {
    capability: CapabilityOffer,
    resource: ResourceOffer,
    state: VectorIndexState,
    handle: VectorIndexHandle,
    backend: HostedHnswVectorIndex<String>,
    session: VectorSearchSession,
}

impl HnswVectorSearchAdapter {
    pub fn new(
        state: VectorIndexState,
        handle: VectorIndexHandle,
        backend: HostedHnswVectorIndex<String>,
    ) -> Result<Self, String> {
        let capability = hosted_hnsw_vector_search_offer(backend.provider(), backend.profile())
            .map_err(|error| format!("HNSW vector-search offer: {error:?}"))?;
        let resource = state
            .contract
            .planning_offer()
            .map_err(|error| format!("HNSW vector-search resource: {error:?}"))?;
        if state.contract.generation != backend.generation()
            || handle.generation != backend.generation()
        {
            return Err("HNSW vector-search adapter generation is stale".into());
        }
        Ok(Self {
            capability,
            resource,
            state,
            handle,
            backend,
            session: VectorSearchSession::default(),
        })
    }

    pub fn mark_provider_lost(&mut self) -> Result<u64, HostedHnswRefusal> {
        self.backend
            .mark_provider_lost(&mut self.state, &self.handle)
    }
}

impl HostedVectorSearchAdapter for HnswVectorSearchAdapter {
    fn capability_offer(&self) -> &CapabilityOffer {
        &self.capability
    }

    fn resource_offer(&self) -> &ResourceOffer {
        &self.resource
    }

    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> HostedVectorSearchTerminal {
        let Ok(request) = self.session.begin() else {
            return HostedVectorSearchTerminal::QueueFull;
        };
        let result = execute_hnsw(
            &self.capability,
            &self.state,
            &self.handle,
            &mut self.backend,
            placement,
            input,
            output,
        );
        if self.session.complete(request).is_err() {
            return HostedVectorSearchTerminal::Cancelled;
        }
        result
    }

    fn cancel(&mut self) {
        self.session.cancel();
    }
}

fn execute_hnsw(
    capability: &CapabilityOffer,
    state: &VectorIndexState,
    handle: &VectorIndexHandle,
    backend: &mut HostedHnswVectorIndex<String>,
    placement: &PlannedGear,
    input: &[u8],
    output: &mut Vec<u8>,
) -> HostedVectorSearchTerminal {
    let Ok((query, admission, binding)) = decode_request(capability, placement, input) else {
        return HostedVectorSearchTerminal::MalformedInput;
    };
    match backend.query(state, handle, &query, admission, binding) {
        Ok(result) => encode_output(
            VectorSearchValue {
                proof_class: VectorSearchExecutionProofClass::Approximate,
                index_generation: result.index_generation,
                admitted_work_units: result.admitted_work_units,
                candidate_count: result.approximate_candidate_count,
                hits: result.hits,
            },
            output,
        ),
        Err(HostedHnswRefusal::ProviderLost) => HostedVectorSearchTerminal::ProviderLost,
        Err(_) => HostedVectorSearchTerminal::Refused,
    }
}

fn decode_request<'a>(
    capability: &CapabilityOffer,
    placement: &'a PlannedGear,
    input: &[u8],
) -> Result<
    (
        SimilarityQuery,
        VectorIndexQueryAdmission,
        &'a ResourceBinding,
    ),
    (),
> {
    if input.len() > MAXIMUM_VECTOR_SEARCH_INPUT_BYTES as usize
        || placement.capability_id != capability.capability_id
        || placement.implementation_id != capability.implementation.implementation_id
        || placement.artifact_id != capability.implementation.artifact_id
        || placement.execution_profile_id != capability.implementation.execution_profile_id
        || placement.kind_id != capability.kind_id
        || placement.inputs != capability.inputs
        || placement.outputs != capability.outputs
        || placement.host_operations != capability.host_operations
    {
        return Err(());
    }
    let count = |key: &str| {
        placement.configuration.iter().find_map(|entry| {
            (entry.key.as_str() == key)
                .then_some(&entry.value)
                .and_then(|value| match value {
                    conduit_core::ConfigurationValue::U64(value) => u32::try_from(*value).ok(),
                    _ => None,
                })
        })
    };
    let admission = VectorIndexQueryAdmission {
        work_units: count("maximum-query-work-units").ok_or(())?,
        maximum_results: count("maximum-results").ok_or(())?,
        concurrent_queries: 1,
    };
    let binding = placement
        .resources
        .iter()
        .find(|binding| binding.class_id.as_str() == VECTOR_SEARCH_RESOURCE_CLASS)
        .ok_or(())?;
    let query = serde_json::from_slice(input).map_err(|_| ())?;
    Ok((query, admission, binding))
}

fn encode_output(
    result: VectorSearchValue<String>,
    output: &mut Vec<u8>,
) -> HostedVectorSearchTerminal {
    output.clear();
    let Ok(encoded) = serde_json::to_vec(&result) else {
        return HostedVectorSearchTerminal::Failed;
    };
    if encoded.len() > MAXIMUM_VECTOR_SEARCH_OUTPUT_BYTES as usize {
        return HostedVectorSearchTerminal::Failed;
    }
    output.extend_from_slice(&encoded);
    HostedVectorSearchTerminal::Produced
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_active_request_refuses_pressure_and_cancel_rejects_late_completion() {
        let mut session = VectorSearchSession::default();
        let request = session.begin().unwrap();
        assert_eq!(session.begin(), Err(VectorSearchSessionRefusal::QueueFull));
        assert_eq!(session.cancel(), Some(request));
        assert_eq!(
            session.complete(request),
            Err(VectorSearchSessionRefusal::CancelledLateCompletion)
        );
        assert_eq!(
            session.complete(request + 1),
            Err(VectorSearchSessionRefusal::StaleCompletion)
        );
    }
}
