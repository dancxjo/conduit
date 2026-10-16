//! Modeled GPU/CPU offers, not physical device or inference timing evidence.
mod common;
use conduit_ai::{LocalModelKindProfile, LOCAL_MODEL_MEMORY_RESOURCE};
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use std::collections::BTreeMap;

fn forecast(ms: i128) -> FulfillmentCostForecast {
    let mut work = ExactNumericWork::new(100_000_000);
    let mut point = |n| {
        let duration = CostDuration::admit(
            RationalQuantity::new(
                ExactRational::from_i128(n),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap(),
            &mut work,
        )
        .unwrap();
        CostDurationRange::admit(duration, duration, &mut work).unwrap()
    };
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(point(0)),
            queue: Some(point(0)),
            service: Some(point(ms)),
            transfer: Some(point(0)),
        },
        &mut work,
    )
    .unwrap()
}

fn exercise(
    gpu_memory: u32,
    cpu_memory: u32,
    intent: PerformanceIntent,
) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
    exercise_observed(gpu_memory, gpu_memory, cpu_memory, cpu_memory, intent)
}

fn exercise_observed(
    gpu_memory: u32,
    gpu_available: u32,
    cpu_memory: u32,
    cpu_available: u32,
    intent: PerformanceIntent,
) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    let source = format!("plot models {{\n{}\n}}\n", ["first", "second"].map(|name| format!(
        " {name}: llm/generate(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = 1024, maximum-work-units = 4096, maximum-history-items = 0)"
    )).join("\n"));
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profile).unwrap();
    let mut hosts = ["gpu", "cpu"].map(|id| {
        common::local_model::local_model_provider(
            id,
            16,
            (2, 4, 6),
            LocalModelKindProfile::Generate,
        )
        .advertisement
    });
    for (index, host) in hosts.iter_mut().enumerate() {
        // Two independent inference slots and ample compute/queue capacity:
        // only the eight-MiB per-instance model-memory reservation is scarce.
        for resource in &mut host.resources {
            if resource.class_id.as_str() == LOCAL_MODEL_MEMORY_RESOURCE {
                resource.capacity_units = if index == 0 { gpu_memory } else { cpu_memory };
            } else if resource.class_id.as_str() == conduit_ai::LOCAL_MODEL_INFERENCE_SLOT_RESOURCE
            {
                resource.capacity_units = 2;
            } else if resource.class_id.as_str() == conduit_ai::LOCAL_MODEL_QUEUE_ITEM_RESOURCE {
                resource.capacity_units = 4;
            } else if resource.class_id.as_str() == conduit_ai::LOCAL_MODEL_QUEUE_KIB_RESOURCE {
                resource.capacity_units = 16;
            }
        }
        host.capabilities[0].limits.max_active_instances = 2;
    }
    let windows: Vec<_> = hosts
        .iter()
        .map(|host| {
            let clock = MonotonicClockIdentity::new(
                host.host_id.clone(),
                host.boot_id.clone(),
                "global-model-memory/clock".into(),
                TemporalScale::Milliseconds,
                1,
                0,
            )
            .unwrap();
            CostObservationWindow::admit(
                MonotonicInstant::new(100, clock.clone()).unwrap(),
                MonotonicInstant::new(200, clock).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let now: Vec<_> = windows
        .iter()
        .map(|w| MonotonicInstant::new(101, w.observed().clock().clone()).unwrap())
        .collect();
    let calibration_id = ArtifactId::from("fixture/global-model-cost@1");
    let calibrations = [CostCalibrationRevision::new(&calibration_id, 1).unwrap()];
    let contexts: Vec<_> = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| CostEvaluationContext {
            host,
            now,
            calibrations: &calibrations,
            execution_observations: &[],
        })
        .collect();
    let requirements = HardRealizationRequirements::default();
    let policy = RealizationPolicy::default();
    let kind = KindId::from(conduit_ai::GENERATION_REQUEST_VALUE_KIND);
    let revision = ArtifactId::from("fixture/global-model-utf8@1");
    let codecs = [CostInputCodec::admit(&kind, &revision, 4096, |bytes| {
        conduit_core::validate_primitive_info(TEXT_INFO_ID, bytes)
            .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
    })
    .unwrap()];
    let value = ValuePayload {
        value_kind: kind.clone(),
        encoded: b"prompt".to_vec(),
    };
    let inputs: Vec<_> = plot
        .gears
        .iter()
        .map(|gear| {
            [CostInput {
                port: &gear.inputs[0].port_id,
                value: &value,
            }]
        })
        .collect();
    let demands: Vec<_> = plot
        .gears
        .iter()
        .zip(&inputs)
        .map(|(gear, inputs)| CostDemand::admit_with_catalog(gear, inputs, &[], &codecs).unwrap())
        .collect();
    let signs: [SignId; 2] = ["fixture/gpu-cost", "fixture/cpu-cost"].map(Into::into);
    let sources: Vec<_> = hosts
        .iter()
        .enumerate()
        .map(|(i, host)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                &signs[i],
                calibrations[0],
                windows[i].clone(),
            )
            .unwrap()
        })
        .collect();
    let estimates = [forecast(1), forecast(10)];
    let bound: Vec<Vec<_>> = demands
        .iter()
        .map(|demand| {
            sources
                .iter()
                .zip(&estimates)
                .map(|(source, estimate)| CostedForecast::bind(source, *demand, estimate).unwrap())
                .collect()
        })
        .collect();
    let operations: Vec<_> = demands
        .iter()
        .zip(&bound)
        .map(|(demand, forecasts)| CostPlanningOperation {
            demand: *demand,
            forecasts,
            requirements: &requirements,
            tie_policy: &policy,
            intent,
            unavailable: UnavailableCostPolicy::RequireKnown,
        })
        .collect();
    let mut observations = common::resource_observations(&hosts);
    for observation in &mut observations {
        if observation.class_id.as_str() == LOCAL_MODEL_MEMORY_RESOURCE {
            observation.unreserved_units = if observation.host_id == hosts[0].host_id {
                gpu_available
            } else {
                cpu_available
            };
        }
    }
    let timed: Vec<_> = observations
        .iter()
        .map(|observation| {
            let index = hosts
                .iter()
                .position(|h| h.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, windows[index].clone()).unwrap()
        })
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    plan_global_realizations_with_costs(
        &plot,
        CostPlanningInputs {
            hosts: &hosts,
            contexts: &contexts,
            operations: &operations,
            advertisements: &[],
            observations: &timed,
            bases: &[],
            options: PlanningOptions {
                connection_bases: &empty,
                line_candidates: &lines,
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        },
        GlobalPlacementBudget { maximum_states: 32 },
        &mut ExactNumericWork::new(100_000_000),
    )
}

#[test]
fn two_model_instances_cannot_share_one_models_gpu_memory_reservation() {
    let result = exercise(8, 8, PerformanceIntent::ThroughputBatch).unwrap();
    assert!(verify_plan(&result.plan));
    let mut reserved = BTreeMap::<HostId, u32>::new();
    for placement in result.plan.fragments.iter().flat_map(|f| &f.placements) {
        let memory = placement
            .resources
            .iter()
            .find(|r| r.class_id.as_str() == LOCAL_MODEL_MEMORY_RESOURCE)
            .unwrap();
        assert_eq!(memory.units, 8);
        *reserved.entry(placement.host_id.clone()).or_default() += memory.units;
    }
    assert_eq!(
        reserved,
        BTreeMap::from([("host/gpu".into(), 8), ("host/cpu".into(), 8)])
    );
    assert!(result.capacity_pruned_prefixes > 0);
}

#[test]
fn no_cpu_memory_fallback_refuses_the_entire_two_model_plan() {
    assert!(matches!(
        exercise(8, 7, PerformanceIntent::ThroughputBatch),
        Err(GlobalPlacementRefusal::NoFeasibleAssignment {
            disposition: GlobalPlacementDisposition::Exhaustive,
            ..
        })
    ));
}

#[test]
fn ample_gpu_memory_selects_both_cheaper_model_instances() {
    let result = exercise(16, 8, PerformanceIntent::ThroughputBatch).unwrap();
    assert!(verify_plan(&result.plan));
    let placements: Vec<_> = result
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .collect();
    assert_eq!(placements.len(), 2);
    assert!(placements.iter().all(|p| p.host_id.as_str() == "host/gpu"));
    assert_eq!(result.score.unknown_operations, 0);
    assert_eq!(
        result.score.known_upper_seconds,
        ExactRational::ratio(2, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap()
    );
}

#[test]
fn favorable_forecasts_cannot_certify_a_deadline_request() {
    let result = exercise(16, 16, PerformanceIntent::BoundedResponse);
    let Err(GlobalPlacementRefusal::Candidate(CostSelectionError::Policy(refusal))) = result else {
        panic!("uncertified deadline must refuse before global search");
    };
    assert_eq!(refusal.reason, CostEvidenceRefusal::UnsupportedIntent);
    assert_eq!(refusal.intent, PerformanceIntent::BoundedResponse);
}

#[test]
fn fresh_memory_withdrawal_overrides_advertised_capacity_and_preserves_old_plan() {
    let old = exercise_observed(16, 16, 16, 16, PerformanceIntent::ThroughputBatch).unwrap();
    let old_plan = old.plan.clone();
    let next = exercise_observed(16, 8, 16, 16, PerformanceIntent::ThroughputBatch).unwrap();
    assert!(verify_plan(&next.plan));
    assert_ne!(next.plan.plan_id, old_plan.plan_id);
    let mut memory = BTreeMap::<HostId, u32>::new();
    for placement in next.plan.fragments.iter().flat_map(|f| &f.placements) {
        let binding = placement
            .resources
            .iter()
            .find(|r| r.class_id.as_str() == LOCAL_MODEL_MEMORY_RESOURCE)
            .unwrap();
        *memory.entry(placement.host_id.clone()).or_default() += binding.units;
    }
    assert_eq!(
        memory,
        BTreeMap::from([("host/gpu".into(), 8), ("host/cpu".into(), 8)])
    );
    assert_eq!(
        next.score.known_upper_seconds,
        ExactRational::ratio(11, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap()
    );
    assert!(matches!(
        exercise_observed(16, 8, 16, 7, PerformanceIntent::ThroughputBatch),
        Err(GlobalPlacementRefusal::NoFeasibleAssignment { .. })
    ));
    assert_eq!(old.plan, old_plan);
    assert!(verify_plan(&old.plan));
}
