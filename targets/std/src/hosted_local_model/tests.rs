use super::*;
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_ai::{
    LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelIdentity,
    LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
};
use conduit_core::{
    BootId, HostId, OfferGeneration, PoolRealizationEnvelope, PoolRealizationHealth,
    ResourceBinding, SignId,
};
use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
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

    fn current_pool_health(&self) -> PoolRealizationHealth {
        PoolRealizationHealth::Ready
    }

    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        output.clear();
        if placement.kind_id.as_str() == conduit_ai::GENERATE_TEXT_KIND {
            output.extend_from_slice(b"The upstairs temperature is 21 degrees Celsius.");
            self.calls.push(placement.kind_id.as_str().into());
            return self.terminal;
        }
        let encoded = match placement.kind_id.as_str() {
            conduit_ai::LLM_GENERATE_KIND => {
                b"The upstairs temperature is 21 degrees Celsius.".to_vec()
            }
            conduit_ai::LLM_CLASSIFY_KIND => {
                serde_json::to_vec(&conduit_ai::FiniteClassification {
                    label: "conduit".into(),
                    allowed_labels: vec!["conduit".into(), "other".into()],
                })
                .unwrap()
            }
            conduit_ai::LLM_EXTRACT_KIND => serde_json::to_vec(&conduit_ai::ValidatedExtraction {
                schema_identity: "fixture/subject@1".into(),
                fields: vec![conduit_ai::ExtractedField {
                    key: "subject".into(),
                    value: "Conduit".into(),
                }],
            })
            .unwrap(),
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
        let contract = conduit_ai::llm_contract(placement.kind_id.as_str()).unwrap();
        let result = conduit_ai::ModelDerivedResult {
            provenance: conduit_ai::ModelResultProvenance::ModelDerived,
            payload_kind: contract.result_payload_kind.as_str().into(),
            payload: encoded,
            implementation_identity: "fixture-runtime/fixture-model".into(),
            request_identity: "request/fixture".into(),
            run_identity: "run/fixture".into(),
            confidence: None,
            disposition: match self.terminal {
                LocalModelAdapterTerminal::Produced => conduit_ai::ModelResultDisposition::Produced,
                LocalModelAdapterTerminal::Truncated => {
                    conduit_ai::ModelResultDisposition::Truncated
                }
                _ => {
                    self.calls.push(placement.kind_id.as_str().into());
                    return self.terminal;
                }
            },
            determinism: self.offer.determinism,
            accounting: conduit_ai::ModelWorkAccounting {
                input_bytes: input.len() as u64,
                context_items: 1,
                output_bytes: 0,
                work_units: 1,
                history_items: 0,
            },
        };
        let mut result = result;
        result.accounting.output_bytes = result.payload.len() as u64;
        output.extend_from_slice(&serde_json::to_vec(&result).unwrap());
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
                maximum_output_bytes: 4_096,
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
    assert_eq!(local.len(), 3);
    assert!(local.iter().any(|capability| {
        capability.kind_id.as_str() == conduit_ai::GENERATE_TEXT_KIND
            && capability.inputs == conduit_ai::generate_text_contract().inputs
            && capability.outputs == conduit_ai::generate_text_contract().outputs
    }));
    assert!(host.advertisement().capabilities.iter().any(|capability| {
        capability.implementation.implementation_id.as_str()
            == conduit_std_offers::HOUSE_PROMPT_STD_IMPLEMENTATION
    }));
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
fn local_model_pool_observation_binds_current_provider_and_resource_truth() {
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
    let advertisement = host.advertisement();
    let capability = advertisement
        .capabilities
        .iter()
        .find(|capability| {
            capability.implementation.implementation_id.as_str()
                == conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        })
        .unwrap();
    let resources = advertisement
        .resources
        .iter()
        .map(|offer| ResourceBinding {
            pool_id: offer.pool_id.clone(),
            class_id: offer.class_id.clone(),
            units: 1,
            protected: None,
            compute: None,
            content: None,
        })
        .collect::<Vec<_>>();
    let realization = PoolRealizationEnvelope {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        offer_generation: advertisement.offer_generation,
        capability_id: capability.capability_id.clone(),
        implementation_id: capability.implementation.implementation_id.clone(),
        artifact_id: capability.implementation.artifact_id.clone(),
        member_capacity: 1,
        resources,
        admitted_lines: vec![],
    };
    let resource_sign_ids = realization
        .resources
        .iter()
        .enumerate()
        .map(|(index, _)| SignId::from(format!("sign/resource/{index}")))
        .collect::<Vec<_>>();
    let observation = host
        .observe_local_model_pool_realization(
            &realization,
            SignId::from("sign/provider/current"),
            &resource_sign_ids,
        )
        .unwrap();
    assert_eq!(observation.health, PoolRealizationHealth::Ready);
    assert!(observation.is_current_for(&realization));
    assert_eq!(observation.resources.len(), realization.resources.len());
    assert!(observation.resources.iter().all(|resource| {
        resource.unreserved_units > 0
            && resource.utilized_units == 0
            && resource.host_id == advertisement.host_id
            && resource.boot_id == advertisement.boot_id
    }));

    let mut stale = realization;
    stale.boot_id = BootId::from("boot/restarted");
    assert!(host
        .observe_local_model_pool_realization(
            &stale,
            SignId::from("sign/provider/stale"),
            &resource_sign_ids,
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

    let mut admitted_adapter = FakeLocalModel {
        offer: offer(vec![LocalModelKindProfile::Generate]),
        terminal: LocalModelAdapterTerminal::Produced,
        calls: Vec::new(),
    };
    let mut operation_started = false;
    let mut output = Vec::new();
    assert_eq!(
        crate::local_model_pool_member::execute_pool_member_once(
            Some(&mut admitted_adapter),
            placement,
            &mut operation_started,
            b"Where is the temperature?",
            &mut output,
        ),
        LocalModelAdapterTerminal::Produced
    );
    assert!(!output.is_empty());
    assert_eq!(admitted_adapter.calls.len(), 1);
    assert_eq!(
        crate::local_model_pool_member::execute_pool_member_once(
            Some(&mut admitted_adapter),
            placement,
            &mut operation_started,
            b"do not replay",
            &mut output,
        ),
        LocalModelAdapterTerminal::Refused
    );
    assert!(output.is_empty());
    assert_eq!(admitted_adapter.calls.len(), 1);

    let mut generate_startup = StartupCatalog::new();
    let mut generate_profiles = ProfileCatalog::new();
    conduit_ai::install_generate_text_catalog(&mut generate_startup, &mut generate_profiles)
        .unwrap();
    let generate_source = "form generation (\n prompt: value/text@1 > text: value/text@1\n) {\n model: ai/generate-text\n prompt > model.prompt\n model.text > text\n}\n";
    let generate_checked =
        check_syntax_document(&parse_syntax_document(generate_source), &generate_startup).unwrap();
    let generate_authoring = conduit_form::expand_canonical_form_for_authoring(
        &generate_checked,
        "generation",
        &generate_profiles,
    )
    .unwrap();
    let generate_plan = host
        .plan_expanded_local(&generate_authoring.expanded)
        .unwrap();
    let generated = &generate_plan.fragments[0].placements[0];
    assert_eq!(generated.kind_id.as_str(), conduit_ai::GENERATE_TEXT_KIND);
    assert_eq!(
        generated.implementation_id.as_str(),
        conduit_ai::LOCAL_MODEL_IMPLEMENTATION
    );
    assert_eq!(generated.artifact_id, placement.artifact_id);

    let classify = conduit_ai::llm_contract(conduit_ai::LLM_CLASSIFY_KIND).unwrap();
    let unsupported = format!(
        "form classification (\n request: {} > result: {}\n) {{\n model: {}\n request > model.request\n model.result > result\n}}\n",
        classify.inputs[0].value_kind.as_str(),
        classify.outputs[0].value_kind.as_str(),
        conduit_ai::LLM_CLASSIFY_KIND,
    );
    let checked = check_syntax_document(&parse_syntax_document(&unsupported), &startup).unwrap();
    let authoring =
        conduit_form::expand_canonical_form_for_authoring(&checked, "classification", &profiles)
            .unwrap();
    assert!(host.plan_expanded_local(&authoring.expanded).is_err());
}

#[test]
fn house_prompt_projection_plans_the_exact_std_realization() {
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
    conduit_text::install_text_catalogs(&mut startup, &mut profiles).unwrap();
    conduit_tongues::install_house_conversation_catalog(&mut startup, &mut profiles).unwrap();
    let source = "form prompt-only (\n > detection: AddressDetection\n > context: HouseContext\n request_value: llm/generation-request@1 >\n) {\n request: house/context-to-prompt\n detection > request.detection\n context > request.context\n request.request > request_value\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        conduit_form::expand_canonical_form_for_authoring(&checked, "prompt-only", &profiles)
            .unwrap();
    let plan = host.plan_expanded_local(&authored.expanded).unwrap();
    let placements = &plan.fragments[0].placements;
    assert_eq!(placements.len(), 1);
    assert!(placements.iter().any(|placement| {
        placement.implementation_id.as_str() == conduit_std_offers::HOUSE_PROMPT_STD_IMPLEMENTATION
    }));
}

fn plan_and_play(profile: LocalModelKindProfile) {
    let local_offer = offer(vec![profile]);
    let contract = conduit_ai::llm_contract(profile.kind()).unwrap();
    let mut host = StdHost::new_with_local_model_capabilities(
        config(),
        StdHostComposition::minimal(),
        Box::new(FakeLocalModel {
            offer: local_offer,
            terminal: LocalModelAdapterTerminal::Produced,
            calls: Vec::new(),
        }),
        vec![
            crate::installed_std::test_local_model_io::source_offer(
                contract.inputs[0].value_kind.as_str(),
            ),
            crate::installed_std::test_local_model_io::sink_offer(
                contract.outputs[0].value_kind.as_str(),
            ),
        ],
    )
    .unwrap();

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
        "form run {{\n source: conduit-test/local-model-request\n model: {}(4096, 1, 4096, 4096, 0)\n sink: conduit-test/local-model-result\n source.value > model.request\n model.result > sink.value\n}}\n",
        profile.kind()
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "run", &profiles).unwrap();
    let hosts = vec![host.advertisement().clone()];
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
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut Vec::new(), &mut NoopTimer)
        .unwrap();
    assert!(report.kernel.is_some());
}

#[test]
fn all_five_l3_profiles_execute_through_ordinary_plan_and_play() {
    plan_and_play(LocalModelKindProfile::Generate);
    plan_and_play(LocalModelKindProfile::ClassifyFiniteLabels);
    plan_and_play(LocalModelKindProfile::ExtractValidatedInfo);
    plan_and_play(LocalModelKindProfile::EmbedFiniteVector);
    plan_and_play(LocalModelKindProfile::InterpretSignEvidence);
}

#[cfg(feature = "local-model-proof")]
#[test]
fn checked_house_form_executes_through_the_ordinary_local_model_play() {
    let mut capabilities =
        crate::installed_std::test_local_model_io::house_source_offers().to_vec();
    capabilities.push(crate::installed_std::recorded_speech_operation::offer());
    capabilities.push(crate::installed_std::test_local_model_io::house_text_sink_offer());
    let mut host = StdHost::new_with_local_model_capabilities(
        config(),
        StdHostComposition::minimal(),
        Box::new(FakeLocalModel {
            offer: offer(vec![LocalModelKindProfile::Generate]),
            terminal: LocalModelAdapterTerminal::Produced,
            calls: Vec::new(),
        }),
        capabilities,
    )
    .unwrap();
    let (plan_id, completed) = crate::local_model_proof::run_house(&mut host).unwrap();
    assert!(!plan_id.is_empty());
    assert!(completed);
}
