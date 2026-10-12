//! Actual native admission/execution with an explicit deterministic adapter.
//! No Ollama inference, producer authentication or physical capacity proof.
use super::*;
use crate::{
    hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal},
    StdHost, StdHostComposition, StdHostConfig,
};
use conduit_core::*;
use conduit_planner::resource_cost::*;
use std::sync::{Arc, Mutex};

fn prompt(bytes: &[u8]) -> Result<(), CostEvidenceRefusal> {
    validate_primitive_info(TEXT_INFO_ID, bytes).map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}
fn with_demand(gear: &conduit_plot::CheckedGear, input: &[u8], check: impl FnOnce(CostDemand<'_>)) {
    let value = ValuePayload {
        value_kind: gear.inputs[0].value_kind.clone(),
        encoded: input.to_vec(),
    };
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    let revision = ArtifactId::from("fixture/native-utf8@1");
    let codecs = [CostInputCodec::admit(&value.value_kind, &revision, 4096, prompt).unwrap()];
    check(CostDemand::admit_with_catalog(gear, &inputs, &[], &codecs).unwrap());
}
struct FixtureAdapter {
    offer: conduit_ai::LocalModelOffer,
    calls_and_output_bound: Arc<Mutex<(usize, u64)>>,
    result: Vec<u8>,
}
impl HostedLocalModelAdapter for FixtureAdapter {
    fn offer(&self) -> &conduit_ai::LocalModelOffer {
        &self.offer
    }
    fn execute(
        &mut self,
        placement: &PlannedGear,
        _: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        let mut state = self.calls_and_output_bound.lock().unwrap();
        state.0 += 1;
        state.1 = placement
            .configuration
            .iter()
            .find_map(|entry| match &entry.value {
                ConfigurationValue::U64(value) if entry.key == "maximum-output-bytes" => {
                    Some(*value)
                }
                _ => None,
            })
            .unwrap();
        output.clear();
        output.extend_from_slice(&self.result);
        LocalModelAdapterTerminal::Produced
    }
}
struct NativeFixture {
    host: StdHost,
    gear: conduit_plot::CheckedGear,
    plan: Plan,
    selection: PoolSelectionEvidence,
    consumer: PlacementId,
    state: Arc<Mutex<(usize, u64)>>,
}
fn native_fixture() -> NativeFixture {
    use conduit_ai::*;
    let state = Arc::new(Mutex::new((0, 0)));
    let model = LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "fixture".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "fixture/build@1".into(),
            model_name: "fixture".into(),
            model_content_identity: "fixture/content@1".into(),
            architecture: "fixture".into(),
            parameter_profile: "tiny".into(),
            quantization: "exact".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds::new(4096, 1, 4096, 4096, 0).unwrap(),
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
            maximum_queue_bytes: 16384,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![LocalModelKindProfile::Generate],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    };
    let contract = llm_contract(LLM_GENERATE_KIND).unwrap();
    let result = ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().into(),
        payload: b"result".to_vec(),
        implementation_identity: "fixture/native-adapter".into(),
        request_identity: "fixture/request".into(),
        run_identity: "fixture/run".into(),
        confidence: None,
        disposition: ModelResultDisposition::Produced,
        determinism: model.determinism,
        accounting: ModelWorkAccounting {
            input_bytes: 7,
            context_items: 1,
            output_bytes: 6,
            work_units: 1,
            history_items: 0,
        },
    };
    result.validate(&contract).unwrap();
    let result = serde_json::to_vec(&result).unwrap();
    let host = StdHost::new_with_local_model(
        StdHostConfig {
            host_id: "host/worker".into(),
            boot_id: "boot/worker".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(FixtureAdapter {
            offer: model,
            calls_and_output_bound: state.clone(),
            result,
        }),
    )
    .unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
    let source = "plot fixture (\n request: llm/generation-request@1 >> result: llm/generated-result@1\n) {\n model: llm/generate(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = 1024, maximum-work-units = 4096, maximum-history-items = 0)\n request >> model.request\n model.result >> result\n}\n";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "fixture", &profiles).unwrap();
    let gear = authoring.expanded.gears[0].clone();
    let native = host.plan_expanded_local(&authoring.expanded).unwrap();
    let placement = &native.fragments[0].placements[0];
    // Reuse the explicit two-Host session Plan fixture. Only this native
    // member's Fore/offer/resource facts change; its Lines remain fixtures.
    let (mut plan, mut selection, consumer) = crate::pool_member_sessions::tests::exact_plan();
    for fragment in &mut plan.fragments {
        let pool = &mut fragment.shared_pools[0];
        pool.member_front = gear.checked_front();
        pool.member_limits.queue_byte_capacity = 4096;
        let envelope = &mut pool.realization_envelope[0];
        envelope.capability_id = placement.capability_id.clone();
        envelope.implementation_id = placement.implementation_id.clone();
        envelope.artifact_id = placement.artifact_id.clone();
        envelope.resources = placement.resources.clone();
        for line in &mut envelope.admitted_lines {
            line.binding.limits.maximum_payload_bytes = 4096;
            line.binding.limits.maximum_buffered_bytes = 4096;
            line.binding.limits.maximum_frame_bytes = 8192;
        }
    }
    plan = seal_plan(
        PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.fragments,
    );
    selection.plan_id = plan.plan_id.clone();
    assert!(verify_plan(&plan));
    NativeFixture {
        host,
        gear,
        plan,
        selection,
        consumer,
        state,
    }
}

#[test]
fn actual_native_member_preserves_checked_profile_and_fixed_request_without_play_allocations() {
    let NativeFixture {
        mut host,
        gear,
        plan,
        selection,
        consumer,
        state,
    } = native_fixture();
    let next_selection = PoolSelectionEvidence {
        operation_id: "fixture/next".into(),
        ..selection.clone()
    };
    with_demand(&gear, b"request", |demand| {
        let mut member = host
            .prepare_local_model_pool_member_with_demand(
                &plan,
                selection.clone(),
                &consumer,
                demand,
            )
            .unwrap();
        assert!(host
            .prepare_local_model_pool_member_with_demand(&plan, selection, &consumer, demand)
            .is_err());
        let mut output = Vec::with_capacity(4096);
        let allocations = crate::allocation_probe::begin();
        let terminal = host.execute_local_model_pool_member(&mut member, b"request", &mut output);
        assert_eq!(allocations.finish(), 0);
        assert_eq!(terminal, LocalModelAdapterTerminal::Produced);
        let result: conduit_ai::ModelDerivedResult = serde_json::from_slice(&output).unwrap();
        result
            .validate(&conduit_ai::llm_contract(conduit_ai::LLM_GENERATE_KIND).unwrap())
            .unwrap();
        assert_eq!(result.payload, b"result");
        let receipt = member.prepare_completed_work().unwrap();
        assert_eq!(receipt.realization().configuration, gear.configuration);
        assert_eq!(
            receipt.realization().semantic_contract,
            gear.semantic_contract
        );
        assert_eq!(
            receipt.input_digest(),
            semantic_digest("conduit.std/local-model-completed-input@1", b"request")
        );
        let completed = receipt.completed().clone();
        assert_eq!(*state.lock().unwrap(), (1, 1024));
        assert_eq!(
            host.execute_local_model_pool_member(&mut member, b"changed", &mut output),
            LocalModelAdapterTerminal::Refused
        );
        assert!(output.is_empty());
        assert_eq!(
            member.prepare_completed_work().unwrap().completed(),
            &completed
        );
        host.release_local_model_pool_member(member).unwrap();
        let next = host
            .prepare_local_model_pool_member_with_demand(&plan, next_selection, &consumer, demand)
            .unwrap();
        host.release_local_model_pool_member(next).unwrap();
    });
}
#[test]
fn swapped_request_refuses_before_effect_and_keeps_the_operation_consumed() {
    let NativeFixture {
        mut host,
        gear,
        plan,
        selection,
        consumer,
        state,
    } = native_fixture();
    with_demand(&gear, b"request", |demand| {
        let mut member = host
            .prepare_local_model_pool_member_with_demand(&plan, selection, &consumer, demand)
            .unwrap();
        let mut output = Vec::with_capacity(4096);
        assert_eq!(
            host.execute_local_model_pool_member(&mut member, b"changed", &mut output),
            LocalModelAdapterTerminal::Refused
        );
        assert_eq!(
            member.prepare_completed_work().unwrap().terminal(),
            LocalModelAdapterTerminal::Refused
        );
        assert_eq!(
            host.execute_local_model_pool_member(&mut member, b"request", &mut output),
            LocalModelAdapterTerminal::Refused
        );
        assert_eq!(state.lock().unwrap().0, 0);
        assert!(member.operation_started());
        host.release_local_model_pool_member(member).unwrap();
    });
}
#[test]
fn invalid_configuration_fore_and_initialized_bounds_refuse_before_reservation() {
    for case in 0..3 {
        let NativeFixture {
            mut host,
            mut gear,
            plan,
            selection,
            consumer,
            state,
        } = native_fixture();
        match case {
            0 => gear.configuration.push(gear.configuration[0].clone()),
            1 => gear.kind_contract_revision = "fixture/wrong-contract".into(),
            _ => {
                gear.configuration
                    .iter_mut()
                    .find(|entry| entry.key == "maximum-context-items")
                    .unwrap()
                    .value = ConfigurationValue::U64(2)
            }
        }
        with_demand(&gear, b"request", |demand| {
            assert!(host
                .prepare_local_model_pool_member_with_demand(
                    &plan,
                    selection.clone(),
                    &consumer,
                    demand
                )
                .is_err());
        });
        let NativeFixture { gear: valid, .. } = native_fixture();
        with_demand(&valid, b"request", |demand| {
            let member = host
                .prepare_local_model_pool_member_with_demand(&plan, selection, &consumer, demand)
                .unwrap();
            host.release_local_model_pool_member(member).unwrap();
        });
        assert_eq!(state.lock().unwrap().0, 0);
    }
}

#[test]
fn forecast_prepared_before_native_execution_calibrates_actual_completed_interval() {
    use crate::{ThreadTimer, TimerAdapter};
    let NativeFixture {
        mut host,
        gear,
        plan,
        selection,
        consumer,
        ..
    } = native_fixture();
    with_demand(&gear, b"request", |demand| {
        let mut member = host
            .prepare_local_model_pool_member_with_demand(&plan, selection, &consumer, demand)
            .unwrap();
        let advertised = host.advertisement().clone();
        let offer = advertised
            .capabilities
            .iter()
            .find(|offer| offer.kind_id == gear.kind_id)
            .unwrap();
        let observed = ThreadTimer
            .monotonic_observation(&advertised.host_id, &advertised.boot_id)
            .unwrap();
        let window = |observed: MonotonicInstant| {
            let expiry = MonotonicInstant::new(
                observed.ticks().checked_add(1_000_000).unwrap(),
                observed.clock().clone(),
            )
            .unwrap();
            CostObservationWindow::admit(observed, expiry).unwrap()
        };
        let calibration = ArtifactId::from("fixture/native-calibration@1");
        let revision = CostCalibrationRevision::new(&calibration, 1).unwrap();
        let forecast_sign = SignId::from("fixture/forecast-before-execution");
        let source = CostEvidenceSource::bind(
            &advertised,
            offer,
            None,
            &forecast_sign,
            revision,
            window(observed),
        )
        .unwrap();
        let mut work = ExactNumericWork::new(100_000_000);
        let point = |us, work: &mut ExactNumericWork| {
            let time = CostDuration::admit(
                RationalQuantity::new(
                    ExactRational::from_i128(us),
                    Unit::resolve("us").unwrap(),
                    QuantityRole::Linear,
                )
                .unwrap(),
                work,
            )
            .unwrap();
            CostDurationRange::admit(time, time, work).unwrap()
        };
        let stages = FulfillmentCostForecast::sequential(
            CostForecastStages {
                setup: Some(point(0, &mut work)),
                queue: Some(point(0, &mut work)),
                service: Some(point(1000, &mut work)),
                transfer: Some(point(0, &mut work)),
            },
            &mut work,
        )
        .unwrap();
        let forecast = CostedForecast::bind(&source, demand, &stages).unwrap();
        let mut output = Vec::with_capacity(4096);
        assert_eq!(
            host.execute_local_model_pool_member(&mut member, b"request", &mut output),
            LocalModelAdapterTerminal::Produced
        );
        let receipt = member.prepare_completed_work().unwrap();
        let completion_sign = SignId::from("fixture/native-completion");
        let completed = CostEvidenceSource::bind(
            &advertised,
            offer,
            None,
            &completion_sign,
            revision,
            window(receipt.completed().clone()),
        )
        .unwrap();
        let prepared = receipt
            .prepare_calibration(&forecast, &completed, None, &mut work)
            .unwrap();
        assert_eq!(prepared.record().elapsed(), receipt.elapsed());
        assert_eq!(
            prepared.record().completion().window().observed(),
            receipt.completed()
        );
        assert_eq!(
            prepared.record().condition(),
            CostExecutionCondition::Unknown
        );
        let mut model = BoundedCostCalibration::admit(CostCalibrationPolicy {
            history_capacity: 2,
            maximum_updates: 4,
            minimum_update_ticks: 1,
        })
        .unwrap();
        model.update(prepared.record(), &mut work).unwrap();
        assert_eq!(model.updates(), 1);
        assert!(model
            .estimate_at(&advertised, revision, receipt.completed())
            .unwrap()
            .is_some());
        host.release_local_model_pool_member(member).unwrap();
    });
}
