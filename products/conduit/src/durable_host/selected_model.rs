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
            self.reviewed_offer.supported_profiles.clone(),
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
    if (options.model_generation || !options.model_profiles.is_empty())
        && (options.selected_model.is_none() || options.without_selected_model)
    {
        return Err("model profiles require an explicit model selection".into());
    }
    let profiles = requested_profiles(&options)?;
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
    let adapter = discovery.initialize(admitted_memory_mib, profiles)?;
    let selection = Selection {
        endpoint,
        model_name: adapter.offer().identity.model_name.clone(),
        admitted_memory_mib,
        reviewed_offer: adapter.offer().clone(),
    };
    selection.validate()?;
    Ok(Change::Replace(Box::new(selection)))
}

fn requested_profiles(
    options: &InstalledModelOptions,
) -> Result<Vec<LocalModelKindProfile>, String> {
    if options.model_generation && !options.model_profiles.is_empty() {
        return Err("explicit model profiles conflict with model-generation".into());
    }
    if options.model_profiles.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_KINDS {
        return Err("model profile selection exceeds its finite bound".into());
    }
    let mut profiles = options.model_profiles.clone();
    if profiles.is_empty() {
        profiles.push(LocalModelKindProfile::PresentSemanticFront);
        if options.model_generation {
            profiles.push(LocalModelKindProfile::Generate);
        }
    }
    if profiles
        .iter()
        .enumerate()
        .any(|(i, p)| profiles[i + 1..].contains(p))
    {
        return Err("model profile selection is duplicated".into());
    }
    Ok(profiles)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_cannot_upgrade_a_preserved_or_removed_selection_implicitly() {
        assert!(matches!(
            change(InstalledModelOptions::default()),
            Ok(Change::Preserve)
        ));
        for without_selected_model in [false, true] {
            let result = change(InstalledModelOptions {
                model_generation: true,
                without_selected_model,
                ..Default::default()
            });
            let Err(error) = result else {
                panic!("generation requires explicit reselection")
            };
            assert!(error.contains("explicit model selection"));
        }
    }

    #[test]
    fn explicit_stream_profile_does_not_select_unrelated_presentation() {
        let options = InstalledModelOptions {
            model_profiles: vec![LocalModelKindProfile::StreamGenerate],
            ..Default::default()
        };
        assert_eq!(
            requested_profiles(&options).unwrap(),
            vec![LocalModelKindProfile::StreamGenerate]
        );
        let mut selected = fixture_retained_selection();
        selected.reviewed_offer.supported_profiles = requested_profiles(&options).unwrap();
        selected.validate().unwrap();
        let restored: Selection =
            serde_json::from_slice(&serde_json::to_vec(&selected).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored, selected);
        let offers = restored.reviewed_offer.capability_offers().unwrap();
        assert!(offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == LocalModelKindProfile::StreamGenerate.kind()));
        assert!(!offers
            .iter()
            .any(|offer| offer.kind_id.as_str()
                == LocalModelKindProfile::PresentSemanticFront.kind()));
        assert!(!offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == LocalModelKindProfile::Generate.kind()));
        assert_eq!(
            selected.reviewed_offer.identity,
            fixture_retained_selection().reviewed_offer.identity
        );
        assert_eq!(
            selected.reviewed_offer.limits,
            fixture_retained_selection().reviewed_offer.limits
        );
    }

    #[test]
    fn profile_defaults_duplicates_and_conflicts_are_exact_without_provider_io() {
        assert_eq!(
            requested_profiles(&InstalledModelOptions::default()).unwrap(),
            vec![LocalModelKindProfile::PresentSemanticFront]
        );
        assert_eq!(
            requested_profiles(&InstalledModelOptions {
                model_generation: true,
                ..Default::default()
            })
            .unwrap(),
            vec![
                LocalModelKindProfile::PresentSemanticFront,
                LocalModelKindProfile::Generate
            ]
        );
        let duplicate = InstalledModelOptions {
            model_profiles: vec![LocalModelKindProfile::StreamGenerate; 2],
            ..Default::default()
        };
        assert!(requested_profiles(&duplicate).is_err());
        let conflict = InstalledModelOptions {
            model_generation: true,
            model_profiles: vec![LocalModelKindProfile::StreamGenerate],
            ..Default::default()
        };
        assert!(requested_profiles(&conflict).is_err());
        for without_selected_model in [false, true] {
            assert!(change(InstalledModelOptions {
                without_selected_model,
                model_profiles: vec![LocalModelKindProfile::StreamGenerate],
                ..Default::default()
            })
            .is_err());
        }
        let mut invalid = fixture_retained_selection();
        invalid.reviewed_offer.supported_profiles.clear();
        assert!(invalid.validate().is_err());
        invalid.reviewed_offer.supported_profiles = vec![LocalModelKindProfile::StreamGenerate; 2];
        assert!(invalid.validate().is_err());
        assert!(requested_profiles(&InstalledModelOptions {
            model_profiles: vec![
                LocalModelKindProfile::StreamGenerate;
                conduit_ai::MAXIMUM_LOCAL_MODEL_KINDS + 1
            ],
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn retained_generation_profile_keeps_exact_artifact_and_resource_bounds() {
        let mut selection = fixture_retained_selection();
        selection.validate().unwrap();
        let offers = selection.reviewed_offer.capability_offers().unwrap();
        assert!(!offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == LocalModelKindProfile::Generate.kind()));
        let identity = selection.reviewed_offer.identity.clone();
        let limits = selection.reviewed_offer.limits;
        selection.reviewed_offer.supported_profiles.extend([
            LocalModelKindProfile::Generate,
            LocalModelKindProfile::GenerateFlow,
        ]);
        let bytes = serde_json::to_vec(&selection).unwrap();
        let restored: Selection = serde_json::from_slice(&bytes).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored, selection);
        assert_eq!(restored.reviewed_offer.identity, identity);
        assert_eq!(restored.reviewed_offer.limits, limits);
        let offers = restored.reviewed_offer.capability_offers().unwrap();
        assert!(offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == LocalModelKindProfile::Generate.kind()));
        assert!(offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == LocalModelKindProfile::GenerateFlow.kind()));
        assert!(offers
            .iter()
            .any(|offer| offer.kind_id.as_str()
                == LocalModelKindProfile::PresentSemanticFront.kind()));
    }
}

#[cfg(all(test, unix))]
#[path = "selected_model_provider_tests.rs"]
mod provider_tests;
