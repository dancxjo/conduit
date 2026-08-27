//! Explicit initialized local-model adapter below portable L0 semantics.

use conduit_core::PlannedGear;

mod ollama;
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

pub trait HostedLocalModelAdapter: Send {
    fn offer(&self) -> &conduit_ai::LocalModelOffer;

    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal;
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
mod tests {
    use super::*;
    use crate::{StdHost, StdHostComposition, StdHostConfig};
    use conduit_ai::{
        LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelIdentity,
        LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
    };
    use conduit_core::{BootId, HostId, OfferGeneration};
    use conduit_form::{
        check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog,
    };
    use std::collections::BTreeMap;

    struct FakeLocalModel {
        offer: LocalModelOffer,
        terminal: LocalModelAdapterTerminal,
        calls: Vec<String>,
    }

    impl HostedLocalModelAdapter for FakeLocalModel {
        fn offer(&self) -> &LocalModelOffer {
            &self.offer
        }

        fn execute(
            &mut self,
            placement: &PlannedGear,
            input: &[u8],
            output: &mut Vec<u8>,
        ) -> LocalModelAdapterTerminal {
            output.clear();
            let encoded = match placement.kind_id.as_str() {
                conduit_ai::LLM_GENERATE_KIND => input.to_vec(),
                conduit_ai::LLM_CLASSIFY_KIND => {
                    serde_json::to_vec(&conduit_ai::FiniteClassification {
                        label: "conduit".into(),
                        allowed_labels: vec!["conduit".into(), "other".into()],
                    })
                    .unwrap()
                }
                conduit_ai::LLM_EXTRACT_KIND => {
                    serde_json::to_vec(&conduit_ai::ValidatedExtraction {
                        schema_identity: "fixture/subject@1".into(),
                        fields: vec![conduit_ai::ExtractedField {
                            key: "subject".into(),
                            value: "Conduit".into(),
                        }],
                    })
                    .unwrap()
                }
                conduit_ai::LLM_EMBED_KIND => serde_json::to_vec(&conduit_ai::FiniteEmbedding {
                    profile_identity: "fixture/embedding-3@1".into(),
                    dimensions: 3,
                    values: vec![0.25, -0.5, 1.0],
                })
                .unwrap(),
                conduit_ai::LLM_INTERPRET_KIND => {
                    let request: conduit_ai::InterpretationRequest =
                        serde_json::from_slice(input).unwrap();
                    serde_json::to_vec(&conduit_ai::ModelInterpretation {
                        provenance: conduit_ai::InterpretationProvenance::ModelDerived,
                        hypothesis: "carrier loss likely explains peer unreachability".into(),
                        referenced_evidence: request
                            .evidence
                            .iter()
                            .map(|evidence| evidence.sign_id.clone())
                            .collect(),
                        unresolved_evidence: Vec::new(),
                        confidence: Some(conduit_ai::ProfileReportedConfidence {
                            score_permille: 700,
                        }),
                        implications: vec!["seek a fresh carrier observation".into()],
                        disposition: conduit_ai::InterpretationDisposition::Interpreted,
                    })
                    .unwrap()
                }
                _ => return LocalModelAdapterTerminal::Refused,
            };
            output.extend_from_slice(&encoded);
            self.calls.push(placement.kind_id.as_str().into());
            self.terminal
        }
    }

    fn offer(profiles: Vec<LocalModelKindProfile>) -> LocalModelOffer {
        LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: "fixture-runtime".into(),
                runtime_version: "1".into(),
                runtime_build_identity: "fixture-runtime/build-1".into(),
                model_name: "fixture-model".into(),
                model_content_identity: "sha256-fixture".into(),
                architecture: "fixture".into(),
                parameter_profile: "tiny".into(),
                quantization: "exact".into(),
            },
            limits: LocalModelLimits {
                work: LlmWorkBounds {
                    maximum_input_bytes: 4_096,
                    maximum_context_items: 1,
                    maximum_output_bytes: 1_024,
                    maximum_work_units: 4_096,
                    maximum_history_items: 0,
                },
                model_bytes: 1,
                admitted_memory_mib: 1,
                compute: conduit_ai::LocalModelComputeNeed {
                    minimum_lanes: 1,
                    preferred_lanes: 2,
                    maximum_lanes: 4,
                    minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
                },
                maximum_in_flight: 1,
                maximum_queue_items: 4,
                maximum_queue_bytes: 16_384,
                cancellation_supported: true,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: profiles,
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        }
    }

    fn config() -> StdHostConfig {
        StdHostConfig {
            host_id: HostId::from("host/local-model-test"),
            boot_id: BootId::from("boot/local-model-test"),
            offer_generation: OfferGeneration(1),
        }
    }

    struct NoopTimer;

    impl crate::TimerAdapter for NoopTimer {
        fn wait(&mut self, _duration: std::time::Duration) {}
    }

    #[test]
    fn only_initialized_adapter_capabilities_enter_the_host_advertisement() {
        let host = StdHost::new_with_local_model(
            config(),
            StdHostComposition::minimal(),
            Box::new(FakeLocalModel {
                offer: offer(vec![
                    LocalModelKindProfile::Generate,
                    LocalModelKindProfile::ClassifyFiniteLabels,
                ]),
                terminal: LocalModelAdapterTerminal::Produced,
                calls: Vec::new(),
            }),
        )
        .unwrap();
        let local = host
            .advertisement()
            .capabilities
            .iter()
            .filter(|capability| {
                capability.implementation.implementation_id.as_str()
                    == conduit_ai::LOCAL_MODEL_IMPLEMENTATION
            })
            .collect::<Vec<_>>();
        assert_eq!(local.len(), 2);
        assert!(host.advertisement().resources.iter().any(|resource| {
            resource.class_id.as_str() == conduit_ai::LOCAL_MODEL_MEMORY_RESOURCE
                && resource.capacity_units == 1
        }));
        assert!(local.iter().all(|capability| {
            capability
                .implementation
                .artifact_id
                .as_str()
                .contains("sha256-fixture")
        }));

        let mut unready = offer(vec![LocalModelKindProfile::Generate]);
        unready.initialized = false;
        unready.lifecycle = LocalModelLifecycleState::Discovered;
        assert!(StdHost::new_with_local_model(
            config(),
            StdHostComposition::minimal(),
            Box::new(FakeLocalModel {
                offer: unready,
                terminal: LocalModelAdapterTerminal::Produced,
                calls: Vec::new(),
            }),
        )
        .is_err());
    }

    #[test]
    fn ordinary_form_planning_selects_only_the_exact_local_model_offer() {
        let host = StdHost::new_with_local_model(
            config(),
            StdHostComposition::minimal(),
            Box::new(FakeLocalModel {
                offer: offer(vec![LocalModelKindProfile::Generate]),
                terminal: LocalModelAdapterTerminal::Produced,
                calls: Vec::new(),
            }),
        )
        .unwrap();
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
        let source = "form generation (\n request: llm/generation-request@1 > result: llm/generated-result@1\n) {\n model: llm/generate\n request > model.request\n model.result > result\n}\n";
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        let authoring =
            conduit_form::expand_canonical_form_for_authoring(&checked, "generation", &profiles)
                .unwrap();
        let plan = host.plan_expanded_local(&authoring.expanded).unwrap();
        assert_eq!(plan.fragments.len(), 1);
        assert_eq!(plan.fragments[0].placements.len(), 1);
        let placement = &plan.fragments[0].placements[0];
        assert_eq!(placement.kind_id.as_str(), conduit_ai::LLM_GENERATE_KIND);
        assert_eq!(
            placement.implementation_id.as_str(),
            conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        );
        assert!(placement.artifact_id.as_str().contains("sha256-fixture"));

        let classify = conduit_ai::llm_contract(conduit_ai::LLM_CLASSIFY_KIND).unwrap();
        let unsupported = format!(
            "form classification (\n request: {} > result: {}\n) {{\n model: {}\n request > model.request\n model.result > result\n}}\n",
            classify.inputs[0].value_kind.as_str(),
            classify.outputs[0].value_kind.as_str(),
            conduit_ai::LLM_CLASSIFY_KIND,
        );
        let checked =
            check_syntax_document(&parse_syntax_document(&unsupported), &startup).unwrap();
        let authoring = conduit_form::expand_canonical_form_for_authoring(
            &checked,
            "classification",
            &profiles,
        )
        .unwrap();
        assert!(host.plan_expanded_local(&authoring.expanded).is_err());
    }

    fn plan_and_play(profile: LocalModelKindProfile) {
        let local_offer = offer(vec![profile]);
        let contract = conduit_ai::llm_contract(profile.kind()).unwrap();
        let mut advertisement =
            StdHost::new_with_composition(config(), StdHostComposition::minimal())
                .advertisement()
                .clone();
        advertisement
            .resources
            .extend(resource_offers(&local_offer.limits));
        advertisement.resources.sort();
        advertisement
            .capabilities
            .extend(local_offer.capability_offers().unwrap());
        advertisement.capabilities.extend([
            crate::installed_std::test_local_model_io::source_offer(
                contract.inputs[0].value_kind.as_str(),
            ),
            crate::installed_std::test_local_model_io::sink_offer(
                contract.outputs[0].value_kind.as_str(),
            ),
        ]);

        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
        crate::installed_std::test_local_model_io::install_catalog(
            &mut startup,
            &mut profiles,
            contract.inputs[0].value_kind.as_str(),
            contract.outputs[0].value_kind.as_str(),
        );
        let source = format!(
            "form run {{\n source: conduit-test/local-model-request\n model: {}(4096, 1, 1024, 4096, 0)\n sink: conduit-test/local-model-result\n source.value > model.request\n model.result > sink.value\n}}\n",
            profile.kind()
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form(&checked, "run", &profiles).unwrap();
        let hosts = vec![advertisement.clone()];
        let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
        let connection_bases = BTreeMap::new();
        let line_candidates = BTreeMap::new();
        let plan = conduit_planner::plan_expanded_canonical_with_options(
            &expanded,
            &hosts,
            &placements,
            &[conduit_core::BaseImplementationId::from(
                "conduit.base/local@1",
            )],
            conduit_planner::PlanningOptions {
                connection_bases: &connection_bases,
                line_candidates: &line_candidates,
                connection_item_capacity: 4,
                connection_byte_capacity: 4_096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
        let mut adapter = FakeLocalModel {
            offer: local_offer,
            terminal: LocalModelAdapterTerminal::Produced,
            calls: Vec::new(),
        };
        let mut sign_sequence = 0;
        crate::installed_std::run_fragment(
            crate::installed_std::InstalledRunHost {
                advertisement: &advertisement,
                playback: None,
                midi_input: None,
                midi_output: None,
                keyboard: None,
                local_model: Some(&mut adapter),
                vector_search: None,
                calendar: None,
            },
            &plan.fragments[0],
            1,
            &mut sign_sequence,
            &mut Vec::new(),
            &mut NoopTimer,
            &crate::RunControl::default(),
        )
        .unwrap();
        assert_eq!(adapter.calls, vec![profile.kind().to_string()]);
    }

    #[test]
    fn all_five_l3_profiles_execute_through_ordinary_plan_and_play() {
        plan_and_play(LocalModelKindProfile::Generate);
        plan_and_play(LocalModelKindProfile::ClassifyFiniteLabels);
        plan_and_play(LocalModelKindProfile::ExtractValidatedInfo);
        plan_and_play(LocalModelKindProfile::EmbedFiniteVector);
        plan_and_play(LocalModelKindProfile::InterpretSignEvidence);
    }
}
