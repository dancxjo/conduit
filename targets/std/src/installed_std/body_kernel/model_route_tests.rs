//! Fenced fake-provider proof through a sealed, shared Body Plan and Play.
use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal},
    ExternalForeDelivery, ExternalForeInput, RunControl, RunControlRequestId, StdHost,
    StdHostComposition, StdHostConfig, TimerAdapter,
};
use conduit_ai::*;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{ConnectionTrack, Plan, PortDirection, SignId, TerminalDisposition};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

const MODEL: &str = "plot model-proof (\n >> requests: GenerationRequest...|\n results: GeneratedResult...| >>\n) {\n model: llm/generate-flow(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = 512, maximum-work-units = 4096, maximum-history-items = 0)\n requests >> model.request\n model.result >> results\n}\n";

struct Fake {
    offer: LocalModelOffer,
    calls: Arc<AtomicUsize>,
    lost: bool,
}
impl HostedLocalModelAdapter for Fake {
    fn offer(&self) -> &LocalModelOffer {
        &self.offer
    }
    fn current_pool_health(&self) -> conduit_core::PoolRealizationHealth {
        conduit_core::PoolRealizationHealth::Ready
    }
    fn execute(
        &mut self,
        gear: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        let sequence = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.lost {
            return LocalModelAdapterTerminal::ProviderLost;
        }
        let contract = llm_contract(gear.kind_id.as_str()).unwrap();
        let payload = cognition::reply(input).unwrap_or_else(|| input.to_vec());
        let output_bytes = payload.len() as u64;
        let result = ModelDerivedResult {
            provenance: ModelResultProvenance::ModelDerived,
            payload_kind: contract.result_payload_kind.as_str().into(),
            payload,
            implementation_identity: "fenced-fake/body-model".into(),
            request_identity: format!("request/fake/{sequence}"),
            run_identity: format!("run/fake/{sequence}"),
            confidence: None,
            disposition: ModelResultDisposition::Produced,
            determinism: self.offer.determinism,
            accounting: ModelWorkAccounting {
                input_bytes: input.len() as u64,
                context_items: 1,
                output_bytes,
                work_units: 1,
                history_items: 0,
            },
        };
        result.validate(&contract).unwrap();
        output.extend_from_slice(&serde_json::to_vec(&result).unwrap());
        LocalModelAdapterTerminal::Produced
    }
}
fn offer() -> LocalModelOffer {
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "fenced-fake".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "fake/body-build".into(),
            model_name: "body-model".into(),
            model_content_identity: "sha256-fake-body".into(),
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
                preferred_lanes: 1,
                maximum_lanes: 1,
                minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
            },
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 16384,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![LocalModelKindProfile::GenerateFlow],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    }
}
fn plan(host: &StdHost, source: &str, name: &str) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    startup
        .insert_value_kind_alias(
            "GenerationRequest",
            conduit_core::kind_id(GENERATION_REQUEST_VALUE_KIND),
        )
        .unwrap();
    startup
        .insert_value_kind_alias(
            "GeneratedResult",
            conduit_core::kind_id(GENERATED_RESULT_VALUE_KIND),
        )
        .unwrap();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, name, &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        ("requests", PortDirection::Input),
        ("results", PortDirection::Output),
    ]
    .into_iter()
    .map(|(name, direction)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: conduit_core::port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 16384,
            },
        )
    })
    .collect();
    let no_boundaries = BTreeMap::new();
    plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: if name == "model-proof" { 16384 } else { 64 },
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        if name == "model-proof" {
            &boundaries
        } else {
            &no_boundaries
        },
    )
    .unwrap()
}
fn prepared(lost: bool) -> (StdHost, BodyPlan, conduit_body::Wake, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let host = StdHost::new_with_local_model(
        StdHostConfig {
            host_id: "host/body-model-proof".into(),
            boot_id: "boot/body-model-proof".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::reference(),
        Box::new(Fake {
            offer: offer(),
            calls: calls.clone(),
            lost,
        }),
    )
    .unwrap();
    let plans = [
        plan(&host, MODEL, "model-proof"),
        plan(
            &host,
            include_str!("../../../../../plots/clock/main.conduit"),
            "clock-demo",
        ),
    ];
    let mut body = Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_plot_id.clone(),
        1,
        SignId::from("sign/model-born"),
    )
    .unwrap();
    body = body
        .admit_plot(
            ResidentPlot::new(
                plans[1].source_document_id.clone(),
                plans[1].checked_plot_id.clone(),
            ),
            "sign/model-clock-admitted".into(),
        )
        .unwrap();
    let wake = body.wake(1, "sign/model-wake".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        plans
            .into_iter()
            .map(|plan| BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    (host, plan, wake, calls)
}
struct Capture {
    values: Vec<ModelDerivedResult>,
    delivered: Arc<AtomicUsize>,
}
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.values
            .push(serde_json::from_slice(&output.bytes).unwrap());
        self.delivered.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
struct Clock {
    now: u64,
    waits: usize,
    delivered: Arc<AtomicUsize>,
    control: RunControl,
}
impl TimerAdapter for Clock {
    fn wait(&mut self, duration: Duration) {
        self.now += duration.as_millis().max(1) as u64;
        self.waits += 1;
        if self.waits >= 3 && self.delivered.load(Ordering::SeqCst) == 2 {
            self.control
                .request_stop(RunControlRequestId::new("stop-model-clock-proof").unwrap())
                .unwrap();
        }
        assert!(
            self.waits < 1000,
            "shared Body failed to make bounded test progress"
        );
    }
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.now)
    }
}
fn inputs() -> Vec<ExternalForeInput> {
    [b"first".as_slice(), b"second".as_slice()]
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: conduit_core::port_id("requests"),
            track: ConnectionTrack::Payload,
            bytes: bytes.to_vec(),
        })
        .collect()
}
#[test]
fn ordinary_model_flow_and_standing_clock_share_one_actual_body_play() {
    let (mut host, plan, wake, calls) = prepared(false);
    let control = RunControl::default();
    let delivered = Arc::new(AtomicUsize::new(0));
    let mut capture = Capture {
        values: vec![],
        delivered: delivered.clone(),
    };
    let mut clock = Clock {
        now: 0,
        waits: 0,
        delivered,
        control: control.clone(),
    };
    let mut output = Vec::new();
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &inputs(),
            true,
            &mut capture,
            &mut output,
            &mut clock,
        )
        .unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(report.wake_at_start.body_id, wake.body_id);
    assert_eq!(report.wake_at_start.wake_id, wake.wake_id);
    assert!(report.play.validate_for(&plan));
    assert_eq!(report.partitions.len(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        capture
            .values
            .iter()
            .map(|v| v.payload.as_slice())
            .collect::<Vec<_>>(),
        vec![b"first".as_slice(), b"second".as_slice()]
    );
    assert_ne!(
        capture.values[0].request_identity,
        capture.values[1].request_identity
    );
    assert!(String::from_utf8(output).unwrap().contains("Clock tick"));
    assert!(clock.waits >= 3);
}
#[test]
fn body_model_provider_loss_remains_a_truthful_terminal_failure() {
    let (mut host, plan, wake, calls) = prepared(true);
    let control = RunControl::default();
    let delivered = Arc::new(AtomicUsize::new(0));
    let mut capture = Capture {
        values: vec![],
        delivered: delivered.clone(),
    };
    let mut clock = Clock {
        now: 0,
        waits: 0,
        delivered,
        control: control.clone(),
    };
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &inputs(),
            true,
            &mut capture,
            &mut Vec::new(),
            &mut clock,
        )
        .unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert_eq!(report.kernel_failure.unwrap().detail, 54);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(capture.values.is_empty());
}
#[test]
fn absent_body_model_provider_refuses_before_play_start() {
    let (mut host, plan, wake, _) = prepared(false);
    host.local_model = None;
    let control = RunControl::default();
    let delivered = Arc::new(AtomicUsize::new(0));
    let mut capture = Capture {
        values: vec![],
        delivered: delivered.clone(),
    };
    let mut clock = Clock {
        now: 0,
        waits: 0,
        delivered,
        control: control.clone(),
    };
    let error = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &inputs(),
            true,
            &mut capture,
            &mut Vec::new(),
            &mut clock,
        )
        .unwrap_err();
    assert!(error.contains("no admitted provider"), "{error}");
    assert_eq!(host.next_kernel_play_sequence, 0);
}

#[test]
fn body_model_shared_resources_remain_reserved_by_the_ordinary_ledger() {
    let (mut host, plan, wake, calls) = prepared(false);
    let advertisement = host.advertisement().clone();
    let held = host
        .kernel_resources
        .prepare_and_reserve_plans(
            &advertisement,
            &plan
                .plots
                .iter()
                .map(|part| (&part.plan, false))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let control = RunControl::default();
    let delivered = Arc::new(AtomicUsize::new(0));
    let mut capture = Capture {
        values: vec![],
        delivered: delivered.clone(),
    };
    let mut clock = Clock {
        now: 0,
        waits: 0,
        delivered,
        control: control.clone(),
    };
    let refusal = host.run_body_plan_with_fore_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs(),
        true,
        &mut capture,
        &mut Vec::new(),
        &mut clock,
    );
    assert!(
        refusal.is_err(),
        "an existing reservation cannot be borrowed by a new Play"
    );
    assert_eq!(host.next_kernel_play_sequence, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for reservation in held {
        host.kernel_resources.release(reservation).unwrap();
    }
}

#[path = "model_route_cognition_tests.rs"]
mod cognition;
