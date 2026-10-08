//! An explicitly reviewed local model belongs to a fresh installed Host Boot.
//! A retained selection never substitutes for current provider observation.

use crate::cli::InstalledModelOptions;
use conduit_ai::LocalModelOffer;
use conduit_std_host::hosted_local_model::{
    HostedLocalModelAdapter, LocalModelKindProfile, OllamaDiscovery, OllamaLocalModelAdapter,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    endpoint: String,
    model_name: String,
    admitted_memory_mib: u32,
    reviewed_offer: LocalModelOffer,
}

pub(super) enum Change {
    Preserve,
    Replace(Box<Selection>),
    Remove,
}

impl Selection {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.endpoint.is_empty()
            || self.endpoint.len() > 2048
            || self.model_name.is_empty()
            || self.model_name.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES
            || self.admitted_memory_mib == 0
            || self.admitted_memory_mib > 131_072
            || self.reviewed_offer.identity.model_name != self.model_name
            || self.reviewed_offer.limits.admitted_memory_mib != self.admitted_memory_mib
            || !self
                .reviewed_offer
                .supported_profiles
                .contains(&LocalModelKindProfile::PresentSemanticFront)
        {
            return Err("installed local-model selection violates its exact bounds".into());
        }
        self.reviewed_offer
            .validate()
            .map_err(|error| format!("installed local-model offer is invalid: {error:?}"))
    }

    /// Reobserve and initialize the selected provider before publishing the
    /// Host advertisement or runtime marker. Changed model content, runtime,
    /// limits, or availability refuses this Boot instead of silently offering
    /// another implementation under an old Body Plan.
    pub(super) fn initialize(&self) -> Result<OllamaLocalModelAdapter, String> {
        self.validate()?;
        let discovery = OllamaDiscovery::discover_at(&self.endpoint, &self.model_name)?;
        let adapter = discovery.initialize(
            self.admitted_memory_mib,
            vec![LocalModelKindProfile::PresentSemanticFront],
        )?;
        if adapter.offer() != &self.reviewed_offer {
            return Err(
                "installed local-model provider changed; reselect before a new Boot".into(),
            );
        }
        Ok(adapter)
    }
}

pub(super) fn change(options: InstalledModelOptions) -> Result<Change, String> {
    if options.without_selected_model {
        return Ok(Change::Remove);
    }
    let Some(model_name) = options.selected_model else {
        return Ok(Change::Preserve);
    };
    let endpoint = options
        .model_endpoint
        .ok_or("selected model needs an endpoint")?;
    let admitted_memory_mib = options
        .model_memory_mib
        .ok_or("selected model needs an admitted memory bound")?;
    if model_name.is_empty()
        || model_name.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES
        || endpoint.is_empty()
        || endpoint.len() > 2048
        || admitted_memory_mib == 0
        || admitted_memory_mib > 131_072
    {
        return Err("selected local-model options violate finite bounds".into());
    }
    let discovery = OllamaDiscovery::discover_at(&endpoint, &model_name)?;
    let adapter = discovery.initialize(
        admitted_memory_mib,
        vec![LocalModelKindProfile::PresentSemanticFront],
    )?;
    let selection = Selection {
        endpoint,
        model_name: adapter.offer().identity.model_name.clone(),
        admitted_memory_mib,
        reviewed_offer: adapter.offer().clone(),
    };
    selection.validate()?;
    Ok(Change::Replace(Box::new(selection)))
}

#[cfg(test)]
pub(super) fn fixture_retained_selection() -> Selection {
    use conduit_ai::{
        LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelComputeNeed,
        LocalModelIdentity, LocalModelLifecycleState, LocalModelLimits,
    };
    use conduit_core::ComputeServiceGuarantee;

    let model_name = "fixture-model".to_string();
    Selection {
        endpoint: "http://127.0.0.1:1".into(),
        model_name: model_name.clone(),
        admitted_memory_mib: 1,
        reviewed_offer: LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: "fixture-runtime".into(),
                runtime_version: "1".into(),
                runtime_build_identity: "fixture-runtime/build-1".into(),
                model_name,
                model_content_identity: "sha256-fixture".into(),
                architecture: "fixture".into(),
                parameter_profile: "tiny".into(),
                quantization: "exact".into(),
            },
            limits: LocalModelLimits {
                work: LlmWorkBounds::new(4_096, 1, 4_096, 4_096, 0).unwrap(),
                model_bytes: 1,
                admitted_memory_mib: 1,
                compute: LocalModelComputeNeed {
                    minimum_lanes: 1,
                    preferred_lanes: 2,
                    maximum_lanes: 4,
                    minimum_service_guarantee: ComputeServiceGuarantee::Shared,
                },
                maximum_in_flight: 1,
                maximum_queue_items: 4,
                maximum_queue_bytes: 16_384,
                cancellation_supported: true,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: vec![LocalModelKindProfile::PresentSemanticFront],
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        },
    }
}
