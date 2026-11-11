//! Actual provider loss is recovered in the same Body Play; no fake response.
use super::*;
use conduit_core::{TerminalCategory, TerminalInfo};
use conduit_kernel::KernelEventKind;

struct RecoveredCapture {
    responses: Vec<Vec<u8>>,
    states: Vec<ExperienceAutobiographyState>,
    losses: Vec<TerminalInfo>,
    progress: Arc<AtomicUsize>,
    expected_losses: usize,
}
impl BodyForeOutputAdapter for RecoveredCapture {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        match output.front_port_id.as_str() {
            "responses" => self.responses.push(output.bytes.to_vec()),
            "autobiography" => self
                .states
                .push(ExperienceAutobiographyState::decode(&output.bytes).unwrap()),
            "losses" => self
                .losses
                .push(TerminalInfo::decode(&output.bytes).unwrap()),
            name => return Err(format!("unexpected recovery Fore {name}")),
        }
        if self.responses.len() == 2 - self.expected_losses
            && self.losses.len() == self.expected_losses
            && self.states.last().is_some_and(|s| *s.completed() == 2)
        {
            self.progress.store(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
struct RecoveryClock {
    now: u64,
    waits: usize,
    after_loss: usize,
    control: RunControl,
    progress: Arc<AtomicUsize>,
}
impl TimerAdapter for RecoveryClock {
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.now)
    }
    fn wait(&mut self, duration: Duration) {
        self.now += duration.as_millis().max(1) as u64;
        self.waits += 1;
        if self.progress.load(Ordering::SeqCst) == 1 {
            self.after_loss += 1;
            if self.after_loss == 3 {
                self.control
                    .request_stop(
                        RunControlRequestId::new("stop-after-recovered-model-loss").unwrap(),
                    )
                    .unwrap();
            }
        }
        assert!(
            self.waits < 1000,
            "typed provider-loss feedback did not finish"
        );
    }
}
#[test]
fn provider_loss_preserves_memory_and_independent_clock_in_the_same_body_play() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| run(true))
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn recoverable_model_kind_preserves_successful_two_episode_feedback() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| run(false))
        .unwrap()
        .join()
        .unwrap();
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
            host_id: "host/recovered-model-loss".into(),
            boot_id: "boot/recovered-model-loss".into(),
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
    let plans = [plan(&host, true, true), plan(&host, false, false)];
    let mut body = Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_plot_id.clone(),
        1,
        "sign/recovery-born".into(),
    )
    .unwrap();
    body = body
        .admit_plot(
            ResidentPlot::new(
                plans[1].source_document_id.clone(),
                plans[1].checked_plot_id.clone(),
            ),
            "sign/recovery-clock-admitted".into(),
        )
        .unwrap();
    let wake = body.wake(1, "sign/recovery-wake".into()).unwrap().1;
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
    let mut capture = RecoveredCapture {
        responses: vec![],
        states: vec![],
        losses: vec![],
        progress: progress.clone(),
        expected_losses: usize::from(lose),
    };
    let mut clock = RecoveryClock {
        now: 0,
        waits: 0,
        after_loss: 0,
        control: control.clone(),
        progress,
    };
    let mut output = Vec::new();
    let (events, originals) = events(true);
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
    if report.failure.is_some() {
        eprintln!(
            "responses={} states={:?} events={:?}",
            capture.responses.len(),
            capture
                .states
                .iter()
                .map(|s| *s.completed())
                .collect::<Vec<_>>(),
            report
                .kernel_events
                .iter()
                .rev()
                .take(12)
                .collect::<Vec<_>>()
        );
    }
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
    assert_eq!(
        capture.responses.len(),
        if lose { 1 } else { 2 },
        "loss produces no model response"
    );
    assert_eq!(capture.losses.len(), usize::from(lose));
    if lose {
        assert_eq!(
            capture.losses[0].category(),
            TerminalCategory::ProviderOrResourceLoss
        );
    }
    let last = capture.states.last().unwrap();
    assert_eq!(*last.completed(), 2);
    let records = last.records().get().as_slice();
    assert_eq!(
        records.len(),
        if lose { 1 } else { 2 },
        "failed episode is not fabricated history"
    );
    assert_eq!(*records[0].episode(), 1);
    assert_eq!(records[0].observation(), &originals[0]);
    assert_eq!(
        records[0].response(),
        &ExperienceAutobiographyResponse::model_derived(
            BoundedBytes::new(&capture.responses[0]).unwrap()
        )
        .unwrap()
    );
    let prompts = prompts.lock().unwrap();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[1]["evidence"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["origin"] == "remembered"));
    assert_eq!(
        report
            .kernel_events
            .iter()
            .any(|event| event.kind == KernelEventKind::SemanticAbnormalRecovered),
        lose
    );
    if lose {
        use conduit_kernel::causal_evidence::{
            CausalRelationship, EvidenceMetadataFact, EvidenceMetadataLookup,
            EvidenceMetadataVisit, EvidenceOutcome,
        };
        let causal =
            crate::body_causal_evidence::BodyRunCausalRecord::from_run(&plan, &report).unwrap();
        let recovered = causal
            .retained_evidence()
            .find(|identity| {
                let mut recovered = false;
                let mut exact_play = false;
                assert_eq!(
                    causal.visit(*identity, &mut |fact| {
                        recovered |=
                            fact == EvidenceMetadataFact::Outcome(EvidenceOutcome::Recovered);
                        exact_play |=
                            fact == EvidenceMetadataFact::Play(report.play.active_play_id.as_str());
                        true
                    }),
                    EvidenceMetadataVisit::Visited
                );
                recovered && exact_play
            })
            .expect("recovery retains exact Body Play provenance");
        assert!(causal
            .graph()
            .trace(recovered)
            .unwrap()
            .edges()
            .any(|edge| edge.relationship == CausalRelationship::Corrects));
    }
    assert_eq!(
        clock.after_loss, 3,
        "standing deterministic work continues after actual loss"
    );
    assert!(String::from_utf8(output).unwrap().contains("Clock tick"));
}
