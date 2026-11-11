//! Native observation frames drive Situation and autobiography in one real Body.
//! The model is a fenced fake; no remembered item or model context is injected.
use super::*;
use conduit_core::{BorrowedTemporalInstant, TerminalCategory, TerminalInfo};
use conduit_kernel::KernelEventKind;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};

struct SituationCapture {
    situations: Vec<ExperienceEvidenceRequest>,
    responses: Vec<Vec<u8>>,
    states: Vec<ExperienceAutobiographyState>,
    losses: Vec<TerminalInfo>,
    progress: Arc<AtomicUsize>,
    expected_loss: bool,
}
impl BodyForeOutputAdapter for SituationCapture {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        match output.front_port_id.as_str() {
            "situations" => self
                .situations
                .push(ExperienceEvidenceRequest::decode(&output.bytes).unwrap()),
            "responses" => self.responses.push(output.bytes.to_vec()),
            "autobiography" => self
                .states
                .push(ExperienceAutobiographyState::decode(&output.bytes).unwrap()),
            "losses" => self
                .losses
                .push(TerminalInfo::decode(&output.bytes).unwrap()),
            name => return Err(format!("unexpected situated Body Fore {name}")),
        }
        if self.situations.len() == 2
            && self.responses.len() == 2 - usize::from(self.expected_loss)
            && self.losses.len() == usize::from(self.expected_loss)
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
struct SituationClock {
    now: u64,
    waits: usize,
    after_feedback: usize,
    control: RunControl,
    progress: Arc<AtomicUsize>,
}
impl TimerAdapter for SituationClock {
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.now)
    }
    fn wait(&mut self, duration: Duration) {
        self.now += duration.as_millis().max(1) as u64;
        self.waits += 1;
        if self.progress.load(Ordering::SeqCst) == 1 {
            self.after_feedback += 1;
            if self.after_feedback == 3 {
                self.control
                    .request_stop(RunControlRequestId::new("stop-situated-feedback-proof").unwrap())
                    .unwrap();
            }
        }
        assert!(
            self.waits < 1000,
            "native Situation feedback failed to make progress"
        );
    }
}
fn instant(ticks: u64, uncertainty: u64) -> NativeTemporalInstant {
    NativeTemporalInstant::new(
        "clock/situated-body-proof".into(),
        1,
        NativeTemporalScale::Milliseconds,
        ticks,
        uncertainty,
    )
    .unwrap()
}
fn frames() -> Vec<ExternalForeInput> {
    [("light/first", 95, 0, 100), ("light/second", 185, 7, 200)]
        .into_iter()
        .map(|(id, observed, uncertainty, reference)| {
            let observations = ExperienceSituationObservations::new(
                BoundedSequence::try_from_iter([ExperienceSituationObservation::new(
                    evidence::observation(id),
                    ExperienceSituationTime::at(instant(observed, uncertainty)).unwrap(),
                )
                .unwrap()])
                .unwrap(),
            )
            .unwrap();
            ExternalForeInput {
                front_port_id: "observations".into(),
                track: ConnectionTrack::Payload,
                bytes: ExperienceSituationFrame::new(
                    observations,
                    ExperienceTemporalPolicy::new(10, 20).unwrap(),
                    instant(reference, 0),
                    true,
                    evidence::selection(&[id]),
                )
                .unwrap()
                .encode()
                .unwrap(),
            }
        })
        .collect()
}
fn timing(item: &ExperienceEvidenceItem) -> (u64, u64, u64) {
    for source in item.sources().get().as_slice() {
        let encoded = source.clone().encode().unwrap();
        let view = conduit_core::validate_canonical_structured_value(&encoded).unwrap();
        if let Some(context) = view.variant_payload("temporal_context").unwrap() {
            let observed = BorrowedTemporalInstant::from_canonical(
                context.record_field("observed_at").unwrap().unwrap(),
            )
            .unwrap();
            let reference = BorrowedTemporalInstant::from_canonical(
                context.record_field("reference_at").unwrap().unwrap(),
            )
            .unwrap();
            assert_eq!(observed.clock_basis, "clock/situated-body-proof");
            assert_eq!(reference.clock_basis, observed.clock_basis);
            return (observed.ticks, reference.ticks, observed.uncertainty_ticks);
        }
    }
    panic!("derived Situation must expose the exact observation/reference pair");
}
fn temporal_source(item: &serde_json::Value) -> &serde_json::Value {
    item["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["role"] == "temporal_context")
        .expect("readable typed temporal source")
}
fn run(lose: bool) {
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
    let mut model_offer = offer();
    model_offer.supported_profiles = vec![LocalModelKindProfile::GenerateRecoverableFlow];
    let mut host = StdHost::new_with_local_model_capabilities(
        StdHostConfig {
            host_id: "host/situated-body-proof".into(),
            boot_id: "boot/situated-body-proof".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::reference(),
        Box::new(FencedModel {
            offer: model_offer,
            prompts: prompts.clone(),
            lose_on: lose.then_some(2),
        }),
        vec![state],
    )
    .unwrap();
    let plans = [planning::situated(&host), plan(&host, false, false)];
    let mut body = Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_plot_id.clone(),
        1,
        "sign/situated-body-born".into(),
    )
    .unwrap();
    body = body
        .admit_plot(
            ResidentPlot::new(
                plans[1].source_document_id.clone(),
                plans[1].checked_plot_id.clone(),
            ),
            "sign/situated-body-clock-admitted".into(),
        )
        .unwrap();
    let wake = body.wake(1, "sign/situated-body-wake".into()).unwrap().1;
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
    let mut capture = SituationCapture {
        situations: vec![],
        responses: vec![],
        states: vec![],
        losses: vec![],
        progress: progress.clone(),
        expected_loss: lose,
    };
    let mut clock = SituationClock {
        now: 0,
        waits: 0,
        after_feedback: 0,
        control: control.clone(),
        progress,
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
            &frames(),
            true,
            &mut capture,
            &mut output,
            &mut clock,
        )
        .unwrap();
    assert!(
        matches!(report.terminal, TerminalDisposition::Cancelled { .. }),
        "{:?}: {:?}",
        report.terminal,
        report.failure
    );
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert!(report.kernel_failure.is_none());
    assert!(report.play.validate_for(&plan));
    assert_eq!(report.wake_at_start.body_id, wake.body_id);
    assert_eq!(report.wake_at_start.wake_id, wake.wake_id);
    assert_eq!(report.partitions.len(), 2);
    assert_eq!(capture.situations.len(), 2);
    let first = &capture.situations[0].evidence().get().as_slice()[0];
    let second = &capture.situations[1].evidence().get().as_slice()[0];
    assert_eq!(*first.origin(), ExperienceOrigin::Observation);
    assert_eq!(*first.temporal_role(), ExperienceTemporalRole::Current);
    assert_eq!(*second.origin(), ExperienceOrigin::Observation);
    assert_eq!(
        *second.temporal_role(),
        ExperienceTemporalRole::Indeterminate
    );
    assert_eq!(*second.certainty(), ExperienceCertainty::Uncertain);
    assert_eq!(timing(first), (95, 100, 0));
    assert_eq!(timing(second), (185, 200, 7));
    let prompts = prompts.lock().unwrap();
    assert_eq!(prompts.len(), 2);
    let first_items = prompts[0]["evidence"]["items"].as_array().unwrap();
    assert_eq!(first_items.len(), 1);
    assert_eq!(first_items[0]["temporal_role"], "current");
    assert_eq!(temporal_source(&first_items[0])["observed_at"]["ticks"], 95);
    let second_items = prompts[1]["evidence"]["items"].as_array().unwrap();
    let current = second_items
        .iter()
        .find(|item| item["origin"] == "observation")
        .unwrap();
    let remembered = second_items
        .iter()
        .find(|item| item["origin"] == "remembered")
        .unwrap();
    assert_eq!(current["temporal_role"], "indeterminate");
    assert_eq!(current["certainty"], "uncertain");
    assert_eq!(temporal_source(current)["observed_at"]["ticks"], 185);
    assert_eq!(temporal_source(current)["reference_at"]["ticks"], 200);
    assert_eq!(
        temporal_source(current)["observed_at"]["uncertainty_ticks"],
        7
    );
    assert_eq!(
        temporal_source(current)["policy"]["maximum_current_age_ticks"],
        10
    );
    assert_eq!(
        temporal_source(current)["policy"]["maximum_recent_age_ticks"],
        20
    );
    assert_eq!(remembered["temporal_role"], "historical");
    // Original autobiographical provenance stays immutable. The current item
    // supplies reference 200; the memory preserves its original reference 100.
    assert_eq!(temporal_source(remembered)["observed_at"]["ticks"], 95);
    assert_eq!(temporal_source(remembered)["reference_at"]["ticks"], 100);
    assert!(remembered["sources"]
        .to_string()
        .contains("autobiography/episode/1"));
    assert_eq!(capture.responses.len(), if lose { 1 } else { 2 });
    assert_eq!(capture.losses.len(), usize::from(lose));
    let last = capture.states.last().unwrap();
    assert_eq!(*last.completed(), 2);
    let records = last.records().get().as_slice();
    assert_eq!(records.len(), if lose { 1 } else { 2 });
    assert_eq!(records[0].observation(), &capture.situations[0]);
    assert_eq!(
        timing(&records[0].observation().evidence().get().as_slice()[0]),
        (95, 100, 0)
    );
    assert_eq!(
        records[0].response(),
        &ExperienceAutobiographyResponse::model_derived(
            BoundedBytes::new(&capture.responses[0]).unwrap()
        )
        .unwrap()
    );
    if lose {
        assert_eq!(
            capture.losses[0].category(),
            TerminalCategory::ProviderOrResourceLoss
        );
        assert!(report
            .kernel_events
            .iter()
            .any(|event| event.kind == KernelEventKind::SemanticAbnormalRecovered));
    } else {
        let first: ModelDerivedResult = serde_json::from_slice(&capture.responses[0]).unwrap();
        let second: ModelDerivedResult = serde_json::from_slice(&capture.responses[1]).unwrap();
        assert_ne!(first.payload, second.payload);
        assert_eq!(records[1].observation(), &capture.situations[1]);
    }
    assert_eq!(clock.after_feedback, 3);
    assert!(String::from_utf8(output).unwrap().contains("Clock tick"));
}
#[test]
fn native_frames_derive_situations_and_recall_immutable_temporal_history_in_one_body_play() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| run(false))
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn situated_model_loss_preserves_timed_history_and_independent_clock() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| run(true))
        .unwrap()
        .join()
        .unwrap();
}
