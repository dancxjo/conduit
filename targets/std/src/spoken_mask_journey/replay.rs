//! Correlate a retained Presenter result; this adapter performs no inference.
use conduit_presentation::GenerativePresenterRequest;

pub(super) struct Replay {
    pub(super) offer: conduit_ai::LocalModelOffer,
    pub(super) retained: conduit_presentation::GeneratedManifestationCandidate,
}
impl crate::hosted_local_model::HostedLocalModelAdapter for Replay {
    fn offer(&self) -> &conduit_ai::LocalModelOffer {
        &self.offer
    }
    fn current_pool_health(&self) -> conduit_core::PoolRealizationHealth {
        conduit_core::PoolRealizationHealth::Ready
    }
    fn execute(
        &mut self,
        placement: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> crate::hosted_local_model::LocalModelAdapterTerminal {
        use crate::hosted_local_model::LocalModelAdapterTerminal;
        if placement.kind_id.as_str() != conduit_ai::LLM_PRESENT_KIND {
            return LocalModelAdapterTerminal::Refused;
        }
        let Ok(request) = serde_json::from_slice::<GenerativePresenterRequest>(input) else {
            return LocalModelAdapterTerminal::Failed;
        };
        let mut manifestation = self.retained.clone();
        manifestation.request_identity = request.request_identity;
        manifestation.source_presentation_identity =
            request.semantic_data.source_presentation_identity;
        manifestation.source_presentation_revision =
            request.semantic_data.source_presentation_revision;
        manifestation.template_contract_revision = request.policy.template_contract_revision;
        manifestation.candidate_identity = manifestation.digest();
        match serde_json::to_vec(&manifestation) {
            Ok(bytes) => {
                output.clear();
                output.extend(bytes);
                LocalModelAdapterTerminal::Produced
            }
            Err(_) => LocalModelAdapterTerminal::Failed,
        }
    }
}
pub(super) fn offer(
    retained: &conduit_presentation::GeneratedManifestationCandidate,
) -> conduit_ai::LocalModelOffer {
    use conduit_ai::{
        LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelComputeNeed,
        LocalModelIdentity, LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits,
        LocalModelOffer,
    };
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: retained.provider_identity.clone(),
            runtime_version: "retained-producer-result".into(),
            runtime_build_identity: retained.presenter_implementation_identity.clone(),
            model_name: retained.model_identity.clone(),
            model_content_identity: retained.generation_run_identity.clone(),
            architecture: "retained-manifestation".into(),
            parameter_profile: "exact-result".into(),
            quantization: "producer-owned".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds::reviewed_default(),
            model_bytes: 1,
            admitted_memory_mib: 1,
            compute: LocalModelComputeNeed {
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
            },
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 262_144,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![LocalModelKindProfile::PresentSemanticFront],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    }
}
