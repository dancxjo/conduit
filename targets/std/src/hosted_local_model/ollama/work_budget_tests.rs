//! Finite owner-budget and wire-accounting conformance; no real model service.
use super::*;
use conduit_core::*;

fn placement() -> PlannedGear {
    let host = conduit_ai::llm_generate_base_fixtures()[0]
        .advertisement
        .clone();
    let offer = &host.capabilities[0];
    conduit_core::planned_gear_from_parts! {
        placement_id: "fixture/budget".into(), gear_id: "fixture/budget".into(),
        kind_id: offer.kind_id.clone(), kind_contract_revision: offer.kind_contract_revision.clone(),
        execution_profile_id: offer.implementation.execution_profile_id.clone(), configuration: Vec::new(),
        host_id: host.host_id, boot_id: host.boot_id, offer_generation: host.offer_generation,
        capability_id: offer.capability_id.clone(), implementation_id: offer.implementation.implementation_id.clone(),
        artifact_id: offer.implementation.artifact_id.clone(), base: None,
        realization_characteristics: Vec::new(), limits: offer.limits.clone(),
        inputs: offer.inputs.clone(), outputs: offer.outputs.clone(), semantic_contract: offer.semantic_contract.clone(),
        terminal_transductions: Vec::new(), host_calls: offer.host_calls.clone(),
        resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
    }
}
fn offered() -> LlmWorkBounds {
    LlmWorkBounds::new(8, 1, 1024, 20, 0).unwrap()
}
fn set(gear: &mut PlannedGear, key: &str, value: u64) {
    gear.configuration.push(ConfigurationEntry {
        key: key.into(),
        value: ConfigurationValue::U64(value),
    });
}
#[test]
fn initialized_and_configured_ceilings_intersect_before_effects() {
    let mut gear = placement();
    set(&mut gear, "maximum-input-bytes", 100);
    set(&mut gear, "maximum-context-items", 128);
    set(&mut gear, "maximum-output-bytes", 12);
    set(&mut gear, "maximum-work-units", 3);
    set(&mut gear, "maximum-history-items", 64);
    let budget = FiniteWorkBudget::admit(&gear, offered(), 8).unwrap();
    assert_eq!(budget.bounds.maximum_input_bytes(), 8);
    assert_eq!(budget.bounds.maximum_context_items(), 1);
    assert_eq!(budget.bounds.maximum_history_items(), 0);
    assert_eq!(budget.output_bytes(), 12);
    assert_eq!(budget.decoder_tokens(512), 3);
    assert_eq!(budget.decoder_tokens(2), 2);
    assert!(matches!(
        FiniteWorkBudget::admit(&gear, offered(), 9),
        Err(LocalModelAdapterTerminal::Refused)
    ));
    let fallback = FiniteWorkBudget::admit(&placement(), offered(), 8).unwrap();
    assert_eq!(fallback.output_bytes(), 1024);
    assert_eq!(fallback.decoder_tokens(512), 20);
}
#[test]
fn duplicate_wrong_type_zero_and_unknown_configuration_refuse() {
    for case in 0..5 {
        let mut gear = placement();
        match case {
            0 => {
                set(&mut gear, "maximum-context-items", 0);
            }
            1 => {
                set(&mut gear, "maximum-work-units", 0);
            }
            2 => {
                set(&mut gear, "maximum-output-bytes", 10);
                set(&mut gear, "maximum-output-bytes", 10);
            }
            3 => {
                gear.configuration.push(ConfigurationEntry {
                    key: "maximum-output-bytes".into(),
                    value: ConfigurationValue::Text("10".into()),
                });
            }
            _ => {
                set(&mut gear, "temperature", 1);
            }
        }
        assert!(matches!(
            FiniteWorkBudget::admit(&gear, offered(), 1),
            Err(LocalModelAdapterTerminal::Refused)
        ));
    }
}
#[test]
fn missing_accounting_never_becomes_known_zero_or_saturating_success() {
    let budget = FiniteWorkBudget::admit(&placement(), offered(), 1).unwrap();
    for (prompt, output) in [
        (None, None),
        (Some(0), None),
        (None, Some(0)),
        (Some(u64::MAX), Some(1)),
    ] {
        assert_eq!(
            budget.generation_work(prompt, output),
            Err(LocalModelAdapterTerminal::InvalidStructuredResult)
        );
    }
    assert_eq!(budget.generation_work(Some(0), Some(0)), Ok(0));
    assert_eq!(budget.generation_work(Some(13), Some(7)), Ok(20));
    assert_eq!(
        budget.generation_work(Some(13), Some(8)),
        Err(LocalModelAdapterTerminal::Failed)
    );
    assert_eq!(
        budget.work(None),
        Err(LocalModelAdapterTerminal::InvalidStructuredResult)
    );
    assert_eq!(
        budget.work(Some(21)),
        Err(LocalModelAdapterTerminal::Failed)
    );
}
#[test]
fn native_wire_counts_preserve_absent_null_exact_zero_and_invalid_numbers() {
    let budget = FiniteWorkBudget::admit(&placement(), offered(), 1).unwrap();
    for json in [
        r#"{"response":"text","done":true}"#,
        r#"{"response":"text","done":true,"prompt_eval_count":null,"eval_count":0}"#,
    ] {
        let response: GenerateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            budget.generation_work(response.prompt_eval_count, response.eval_count),
            Err(LocalModelAdapterTerminal::InvalidStructuredResult)
        );
    }
    let response: GenerateResponse = serde_json::from_str(
        r#"{"response":"text","done":true,"prompt_eval_count":0,"eval_count":0}"#,
    )
    .unwrap();
    assert_eq!(response.done, Some(true));
    assert_eq!(
        budget.generation_work(response.prompt_eval_count, response.eval_count),
        Ok(0)
    );
    let embed: EmbedResponse = serde_json::from_str(r#"{"embeddings":[[0.5]]}"#).unwrap();
    assert_eq!(
        budget.work(embed.prompt_eval_count),
        Err(LocalModelAdapterTerminal::InvalidStructuredResult)
    );
    let chat: ChatResponse =
        serde_json::from_str(r#"{"message":{"content":"text"},"done":true}"#).unwrap();
    assert_eq!(
        budget.generation_work(chat.prompt_eval_count, chat.eval_count),
        Err(LocalModelAdapterTerminal::InvalidStructuredResult)
    );
    for count in ["-1", "1.5", "18446744073709551616", "\"1\""] {
        let json =
            format!("{{\"response\":\"text\",\"prompt_eval_count\":{count},\"eval_count\":0}}");
        assert!(serde_json::from_str::<GenerateResponse>(&json).is_err());
    }
}

fn finite_adapter_fixture() -> OllamaLocalModelAdapter {
    OllamaLocalModelAdapter {
        offer: LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: "fixture".into(),
                runtime_version: "1".into(),
                runtime_build_identity: "fixture@1".into(),
                model_name: "fixture".into(),
                model_content_identity: "fixture@1".into(),
                architecture: "fixture".into(),
                parameter_profile: "tiny".into(),
                quantization: "fixture".into(),
            },
            limits: LocalModelLimits {
                work: offered(),
                model_bytes: 1,
                admitted_memory_mib: 1,
                compute: conduit_ai::LocalModelComputeNeed {
                    minimum_lanes: 1,
                    preferred_lanes: 1,
                    maximum_lanes: 1,
                    minimum_service_guarantee: ComputeServiceGuarantee::Shared,
                },
                maximum_in_flight: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: 2048,
                cancellation_supported: false,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: vec![LocalModelKindProfile::Generate],
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        },
        // No provider exists here. Falling through to transport would return
        // ProviderLost, not the required pre-effect Refused outcome.
        endpoint: "http://127.0.0.1:0".into(),
        model_name: "fixture".into(),
        next_request_sequence: 7,
        active_stream: None,
        generation_worker: None,
    }
}

#[test]
fn actual_finite_adapter_refuses_unadmitted_input_before_provider_access() {
    let mut adapter = finite_adapter_fixture();
    let mut output = b"old output".to_vec();
    let gear = placement();
    assert_eq!(
        adapter.execute(&gear, b"oversized", &mut output),
        LocalModelAdapterTerminal::Refused
    );
    assert!(output.is_empty());
    assert_eq!(adapter.next_request_sequence, 7);
    let mut gear = placement();
    set(&mut gear, "maximum-context-items", 0);
    assert_eq!(
        adapter.execute(&gear, b"input", &mut output),
        LocalModelAdapterTerminal::Refused
    );
    assert!(output.is_empty());
    assert_eq!(adapter.next_request_sequence, 7);
}

#[test]
fn worker_repreparation_keeps_attempts_completed_by_other_model_operations() {
    let mut adapter = finite_adapter_fixture();
    adapter
        .offer
        .supported_profiles
        .push(LocalModelKindProfile::GenerateFlow);
    // Preparation-only finite worker fixture: no request is dispatched and no
    // provider is contacted. Placement dispatch is tested through installed Plays.
    let mut flow = placement();
    flow.kind_id = conduit_ai::LLM_GENERATE_FLOW_KIND.into();
    set(&mut flow, "maximum-input-bytes", 8);
    set(&mut flow, "maximum-output-bytes", 1024);
    adapter
        .prepare_background_generation(core::slice::from_ref(&flow))
        .unwrap();
    // A completed non-Flow operation advances the owner after the worker's last
    // observation. Retiring that worker must not make earlier run IDs reusable.
    adapter.next_request_sequence += 2;
    let later = adapter.next_request_sequence;
    adapter
        .prepare_background_generation(core::slice::from_ref(&flow))
        .unwrap();
    assert_eq!(adapter.next_request_sequence, later);
    assert_eq!(
        adapter.generation_worker.as_ref().unwrap().next_sequence,
        later
    );
    adapter.next_request_sequence += 1;
    let final_sequence = adapter.next_request_sequence;
    adapter.prepare_background_generation(&[]).unwrap();
    assert_eq!(adapter.next_request_sequence, final_sequence);
    assert!(adapter.generation_worker.is_none());
}

#[test]
fn worker_dispatch_observes_sequence_advanced_while_idle() {
    let mut adapter = finite_adapter_fixture();
    adapter
        .offer
        .supported_profiles
        .push(LocalModelKindProfile::GenerateFlow);
    let mut flow = placement();
    flow.kind_id = conduit_ai::LLM_GENERATE_FLOW_KIND.into();
    set(&mut flow, "maximum-input-bytes", 8);
    set(&mut flow, "maximum-output-bytes", 1024);
    adapter
        .prepare_background_generation(core::slice::from_ref(&flow))
        .unwrap();
    adapter.next_request_sequence = 9;
    let mut output = Vec::new();
    // Malformed typed input is refused before provider access, but still crosses
    // the real prepared dispatch/completion slot and exercises sequence custody.
    assert_eq!(adapter.begin_generation(&flow, b"x", &mut output), None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let terminal = loop {
        if let Some(terminal) = adapter.poll_generation(&mut output) {
            break terminal;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "worker did not complete"
        );
        std::thread::yield_now();
    };
    assert_ne!(terminal, LocalModelAdapterTerminal::ProviderLost);
    assert_eq!(adapter.next_request_sequence, 9);
    assert_eq!(adapter.generation_worker.as_ref().unwrap().next_sequence, 9);
}
