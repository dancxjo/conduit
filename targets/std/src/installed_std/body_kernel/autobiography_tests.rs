//! Automatic recall/retain feedback in one actual Body, using a fenced fake model.
//! Events supply only present observations; no prior memory is injected by this proof.
use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal},
    ExternalForeDelivery, ExternalForeInput, RunControl, RunControlRequestId, StdHost,
    StdHostComposition, StdHostConfig, TimerAdapter,
};
use conduit_ai::*;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{ConnectionTrack, Plan, PortDirection, TerminalDisposition};
use conduit_human::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_connection_limits,
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::rust_binding::{BoundedBytes, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
#[allow(dead_code)]
#[path = "../../../../../semantics/catalog/tests/common/experience_evidence.rs"]
mod evidence;

struct FencedModel {
    offer: LocalModelOffer,
    prompts: Arc<Mutex<Vec<serde_json::Value>>>,
    lose_on: Option<usize>,
}
impl HostedLocalModelAdapter for FencedModel {
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
        let prompt: serde_json::Value = serde_json::from_slice(input).unwrap();
        let remembered = prompt["evidence"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["origin"] == "remembered");
        let mut prompts = self.prompts.lock().unwrap();
        let sequence = prompts.len() + 1;
        prompts.push(prompt);
        if self.lose_on == Some(sequence) {
            return LocalModelAdapterTerminal::ProviderLost;
        }
        let payload = if remembered {
            b"I remember the earlier light observation.".to_vec()
        } else {
            b"I notice the light for the first time.".to_vec()
        };
        let result = ModelDerivedResult {
            provenance: ModelResultProvenance::ModelDerived,
            payload_kind: llm_contract(gear.kind_id.as_str())
                .unwrap()
                .result_payload_kind
                .as_str()
                .into(),
            accounting: ModelWorkAccounting {
                input_bytes: input.len() as u64,
                context_items: 1,
                output_bytes: payload.len() as u64,
                work_units: 1,
                history_items: 0,
            },
            payload,
            implementation_identity: "fenced-fake/autobiography".into(),
            request_identity: format!("request/autobiography/{sequence}"),
            run_identity: format!("run/autobiography/{sequence}"),
            confidence: None,
            disposition: ModelResultDisposition::Produced,
            determinism: self.offer.determinism,
        };
        result
            .validate(&llm_contract(gear.kind_id.as_str()).unwrap())
            .unwrap();
        output.clear();
        output.extend_from_slice(&serde_json::to_vec(&result).unwrap());
        LocalModelAdapterTerminal::Produced
    }
}
fn offer() -> LocalModelOffer {
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "fenced-fake".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "fake/autobiography-build".into(),
            model_name: "autobiography-model".into(),
            model_content_identity: "sha256-fake-autobiography".into(),
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
mod planning;
use planning::plan;
mod recovery;

struct Capture {
    responses: Vec<Vec<u8>>,
    states: Vec<ExperienceAutobiographyState>,
    progress: Arc<AtomicUsize>,
}
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        match output.front_port_id.as_str() {
            "responses" => self.responses.push(output.bytes.to_vec()),
            "autobiography" => self
                .states
                .push(ExperienceAutobiographyState::decode(&output.bytes).unwrap()),
            name => return Err(format!("unexpected autobiography proof Fore {name}")),
        }
        if self.responses.len() == 2
            && self
                .states
                .last()
                .is_some_and(|state| *state.completed() == 2)
        {
            self.progress.store(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
struct Clock {
    now: u64,
    waits: usize,
    control: RunControl,
    progress: Arc<AtomicUsize>,
}
impl TimerAdapter for Clock {
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.now)
    }
    fn wait(&mut self, duration: Duration) {
        self.now += duration.as_millis().max(1) as u64;
        self.waits += 1;
        if self.waits >= 3 && self.progress.load(Ordering::SeqCst) == 1 {
            self.control
                .request_stop(RunControlRequestId::new("stop-autobiography-proof").unwrap())
                .unwrap();
        }
        assert!(
            self.waits < 1000,
            "autobiography feedback failed to make progress"
        );
    }
}
fn events(retain: bool) -> (Vec<ExternalForeInput>, Vec<ExperienceEvidenceRequest>) {
    let requests = ["light/first", "light/second"]
        .into_iter()
        .map(|id| evidence::request(&[id], vec![evidence::observation(id)]))
        .collect::<Vec<_>>();
    let events = requests
        .iter()
        .map(|observation| ExternalForeInput {
            front_port_id: "events".into(),
            track: ConnectionTrack::Payload,
            bytes: ExperienceAutobiographyEvent::new(observation.clone(), retain)
                .unwrap()
                .encode()
                .unwrap(),
        })
        .collect();
    (events, requests)
}
fn run(retain: bool) {
    let initial = conduit_semantic_catalog::initial_experience_autobiography_state()
        .into_structured()
        .unwrap();
    let state = conduit_std_offers::state_value_large_std_offer(
        "ExperienceAutobiographyState",
        initial.value_type(),
        &initial,
    )
    .unwrap();
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let mut host = StdHost::new_with_local_model_capabilities(
        StdHostConfig {
            host_id: "host/autobiography-proof".into(),
            boot_id: "boot/autobiography-proof".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::reference(),
        Box::new(FencedModel {
            offer: offer(),
            prompts: prompts.clone(),
            lose_on: None,
        }),
        vec![state],
    )
    .unwrap();
    let plans = [plan(&host, true, false), plan(&host, false, false)];
    let mut body = Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_plot_id.clone(),
        1,
        "sign/autobiography-born".into(),
    )
    .unwrap();
    body = body
        .admit_plot(
            ResidentPlot::new(
                plans[1].source_document_id.clone(),
                plans[1].checked_plot_id.clone(),
            ),
            "sign/autobiography-clock-admitted".into(),
        )
        .unwrap();
    let wake = body.wake(1, "sign/autobiography-wake".into()).unwrap().1;
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
    let control = RunControl::default();
    let progress = Arc::new(AtomicUsize::new(0));
    let mut capture = Capture {
        responses: vec![],
        states: vec![],
        progress: progress.clone(),
    };
    let mut clock = Clock {
        now: 0,
        waits: 0,
        control: control.clone(),
        progress,
    };
    let mut output = Vec::new();
    let (events, originals) = events(retain);
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &events,
            true,
            &mut capture,
            &mut output,
            &mut clock,
        )
        .unwrap();
    assert!(
        matches!(report.terminal, TerminalDisposition::Cancelled { .. }),
        "terminal: {:?}; failure: {:?}; responses: {}; states: {:?}",
        report.terminal,
        report.failure,
        capture.responses.len(),
        capture
            .states
            .iter()
            .map(|s| *s.completed())
            .collect::<Vec<_>>()
    );
    assert_eq!(report.wake_at_start.body_id, wake.body_id);
    assert_eq!(report.wake_at_start.wake_id, wake.wake_id);
    assert!(report.play.validate_for(&plan));
    assert_eq!(report.partitions.len(), 2);
    assert_eq!(capture.responses.len(), 2);
    let prompts = prompts.lock().unwrap();
    assert_eq!(prompts.len(), 2);
    assert!(!prompts[0]["evidence"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["origin"] == "remembered"));
    let second_items = prompts[1]["evidence"]["items"].as_array().unwrap();
    let remembered = second_items
        .iter()
        .find(|item| item["origin"] == "remembered");
    assert_eq!(remembered.is_some(), retain);
    let last = capture.states.last().unwrap();
    assert_eq!(*last.completed(), 2);
    if retain {
        let remembered = remembered.unwrap();
        assert_eq!(remembered["temporal_role"], "historical");
        assert!(remembered["sources"]
            .to_string()
            .contains("autobiography/episode/1"));
        let first: ModelDerivedResult = serde_json::from_slice(&capture.responses[0]).unwrap();
        let second: ModelDerivedResult = serde_json::from_slice(&capture.responses[1]).unwrap();
        assert_ne!(first.payload, second.payload);
        let records = last.records().get().as_slice();
        assert_eq!(records.len(), 2);
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record.observation(), &originals[index]);
            assert_eq!(
                record.observation().evidence().get().as_slice()[0].origin(),
                &ExperienceOrigin::Observation
            );
            assert_eq!(
                record.response(),
                &ExperienceAutobiographyResponse::model_derived(
                    BoundedBytes::new(&capture.responses[index]).unwrap()
                )
                .unwrap()
            );
        }
    } else {
        assert!(last.records().get().as_slice().is_empty());
    }
    assert!(String::from_utf8(output).unwrap().contains("Clock tick"));
    assert!(clock.waits >= 3);
}
#[test]
fn selected_first_episode_is_automatically_recalled_in_second_event_in_one_body_play() {
    prepared_run(true);
}
#[test]
fn unselected_episodes_advance_state_without_creating_recalled_history() {
    prepared_run(false);
}

// Prepared native State contains two fixed 64 KiB slots. Debug preparation
// moves them through several validation frames; give this proof a finite stack.
fn prepared_run(retain: bool) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || run(retain))
        .unwrap()
        .join()
        .unwrap();
}
