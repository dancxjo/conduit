//! Deferred-provider fixtures through real Source/Plan/Play and ordinary Host Calls.
//! No native worker, ORT execution, wall-clock race, or physical timing is claimed.
#[path = "common_model_infer/mod.rs"]
mod fixture;
use conduit_ai::{ModelComputeOffer, ModelComputeRefusal, ModelComputeRuntimeIdentity};
use conduit_core::{CancellationReason, ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_data::PreparedNamedTensorBundleView;
use conduit_kernel::KernelEventKind;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    hosted_model_compute::*, hosted_model_infer::PreparedModelInfer, ExternalForeInput, RunControl,
    RunControlDisposition, RunControlRequestId, StdRunReport,
};
use fixture::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Copy)]
enum Mode {
    Complete,
    StopPending,
    StopReady,
    ProviderLost,
}
#[derive(Default)]
struct Counts {
    loads: AtomicUsize,
    executes: AtomicUsize,
    polls: AtomicUsize,
    ready: AtomicUsize,
    lost: AtomicUsize,
    cancels: AtomicUsize,
    unloads: AtomicUsize,
}
#[derive(Clone, Copy)]
struct Exact {
    request: [u8; 32],
    artifact: [u8; 32],
    inputs: [u8; 32],
    work: u64,
}
struct Deferred {
    inner: LinearF32ModelAdapter,
    mode: Mode,
    control: RunControl,
    counts: Arc<Counts>,
    output: Vec<u8>,
    reader: PreparedNamedTensorBundleView,
    exact: Option<Exact>,
    request_polls: usize,
}
impl ModelComputeAdapter for Deferred {
    fn offer(&self) -> &ModelComputeOffer {
        self.inner.offer()
    }
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity {
        self.inner.runtime_identity()
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        self.counts.loads.fetch_add(1, Ordering::SeqCst);
        self.inner.load(identity, bytes)
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        self.counts.executes.fetch_add(1, Ordering::SeqCst);
        self.request_polls = 0;
        let ModelComputeAdapterTerminal::Produced(result) = self.inner.execute(invocation) else {
            return ModelComputeAdapterTerminal::Failed;
        };
        self.exact = Some(Exact {
            request: result.request_identity,
            artifact: result.artifact_identity,
            inputs: result.inputs_identity,
            work: result.consumed_work_units,
        });
        self.output.clear();
        self.output.extend_from_slice(result.outputs.encoded());
        ModelComputeAdapterTerminal::Pending
    }
    fn poll(&mut self) -> ModelComputeAdapterTerminal<'_> {
        self.counts.polls.fetch_add(1, Ordering::SeqCst);
        self.request_polls += 1;
        match self.mode {
            Mode::StopPending | Mode::StopReady => self
                .control
                .request_stop(RunControlRequestId::new("model-pending/stop").unwrap())
                .unwrap(),
            Mode::ProviderLost => {
                self.counts.lost.fetch_add(1, Ordering::SeqCst);
                return ModelComputeAdapterTerminal::ProviderLost;
            }
            Mode::Complete => {}
        }
        if matches!(self.mode, Mode::StopPending)
            || (matches!(self.mode, Mode::Complete) && self.request_polls == 1)
        {
            return ModelComputeAdapterTerminal::Pending;
        }
        let exact = self.exact.take().expect("one exact pending request");
        self.counts.ready.fetch_add(1, Ordering::SeqCst);
        ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
            request_identity: exact.request,
            artifact_identity: exact.artifact,
            checkpoint_identity: None,
            inputs_identity: exact.inputs,
            outputs: self.reader.read(&self.output).unwrap().encoded_ref(),
            consumed_work_units: exact.work,
            runtime: self.inner.runtime_identity(),
        })
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        self.counts.cancels.fetch_add(1, Ordering::SeqCst);
        self.exact = None;
        self.output.clear();
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.counts.unloads.fetch_add(1, Ordering::SeqCst);
        self.inner.unload()
    }
}
fn run(
    mode: Mode,
    items: usize,
) -> (
    Vec<Result<StdRunReport, String>>,
    Outputs,
    Arc<Counts>,
    RunControl,
) {
    let control = RunControl::default();
    let counts = Arc::new(Counts::default());
    let retained_counts = counts.clone();
    let retained_control = control.clone();
    let (mut host, plan) = host_plan(include_str!("fixtures/model-linear.conduit"), |signature| {
        let inner = LinearF32ModelAdapter::new(
            offer(),
            runtime("pending-play"),
            [[2., 0.], [0., -1.]],
            [1., 4.],
        )
        .unwrap()
        .with_signature(&signature)
        .unwrap();
        Box::new(
            PreparedModelInfer::prepare(
                model(signature),
                Deferred {
                    inner,
                    mode,
                    control: retained_control,
                    counts: retained_counts,
                    output: Vec::with_capacity(262144),
                    reader: PreparedNamedTensorBundleView::prepare().unwrap(),
                    exact: None,
                    request_polls: 0,
                },
                requirement(),
                "test/deferred-model-play@1".into(),
            )
            .unwrap(),
        )
    });
    assert_eq!(
        plan.fragments[0].placements[0].host_calls[0]
            .contract_id
            .as_str(),
        conduit_std_host::hosted_model_infer::MODEL_INFER_OPERATION
    );
    let inputs = [ExternalForeInput {
        front_port_id: "inputs".into(),
        track: ConnectionTrack::Payload,
        bytes: bundle(input([2., 3.])).encode().unwrap(),
    }];
    let mut outputs = Outputs::default();
    let mut reports = Vec::with_capacity(items);
    for _ in 0..items {
        reports.push(host.run_external_plot_controlled_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
            &control,
        ));
    }
    // The same prepared adapter serves each separate Value Play. Only owner Drop
    // retires it, so completed runs do not secretly replace preparation.
    assert_eq!(counts.loads.load(Ordering::SeqCst), 1);
    assert_eq!(counts.unloads.load(Ordering::SeqCst), 0);
    drop(host);
    assert_eq!(counts.unloads.load(Ordering::SeqCst), 1);
    (reports, outputs, counts, control)
}
fn count(report: &StdRunReport, kind: KernelEventKind) -> usize {
    report
        .kernel
        .as_ref()
        .unwrap()
        .kernel_sign
        .iter()
        .filter(|event| event.kind == kind)
        .count()
}
fn cancelled(report: &StdRunReport) {
    assert!(matches!(
        report.observations.last().map(|value| &value.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled {
                reason: CancellationReason::OperatorRequested
            }
        })
    ));
    assert_eq!(report.control_receipts.len(), 1);
    assert_eq!(
        report.control_receipts[0].disposition,
        RunControlDisposition::Accepted
    );
    assert_eq!(
        report.control_receipts[0].request_id.as_str(),
        "model-pending/stop"
    );
    assert_eq!(
        report.control_receipts[0].active_play_id,
        report.kernel.as_ref().unwrap().active_play_id
    );
    for kind in [
        KernelEventKind::HostCallRequested,
        KernelEventKind::HostCallCancellationRequested,
        KernelEventKind::CancellationRequested,
        KernelEventKind::RunCancelled,
    ] {
        assert_eq!(count(report, kind), 1);
    }
    assert_eq!(count(report, KernelEventKind::HostCallCompleted), 0);
    let sign = &report.kernel.as_ref().unwrap().kernel_sign;
    let request = sign
        .iter()
        .find(|event| event.kind == KernelEventKind::HostCallRequested)
        .unwrap();
    let cancel = sign
        .iter()
        .find(|event| event.kind == KernelEventKind::HostCallCancellationRequested)
        .unwrap();
    assert_eq!(
        (request.node, request.request),
        (cancel.node, cancel.request)
    );
}
#[test]
fn ordinary_stop_is_drained_while_named_inference_remains_pending() {
    let (mut reports, outputs, counts, _) = run(Mode::StopPending, 1);
    cancelled(&reports.pop().unwrap().unwrap());
    assert!(outputs.0.is_empty());
    assert_eq!(counts.executes.load(Ordering::SeqCst), 1);
    assert_eq!(counts.polls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.cancels.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ready.load(Ordering::SeqCst), 0);
}
#[test]
fn response_ready_stop_cannot_publish_before_scheduler_cancellation() {
    let (mut reports, outputs, counts, _) = run(Mode::StopReady, 1);
    cancelled(&reports.pop().unwrap().unwrap());
    assert!(outputs.0.is_empty());
    assert_eq!(counts.polls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ready.load(Ordering::SeqCst), 1);
    assert_eq!(counts.cancels.load(Ordering::SeqCst), 1);
}
#[test]
fn completed_before_late_stop_keeps_exact_success_and_one_warm_preparation() {
    let (reports, outputs, counts, control) = run(Mode::Complete, 2);
    let reports = reports.into_iter().map(Result::unwrap).collect::<Vec<_>>();
    assert_eq!(reports.len(), 2);
    assert_ne!(
        reports[0].kernel.as_ref().unwrap().active_play_id,
        reports[1].kernel.as_ref().unwrap().active_play_id
    );
    assert_eq!(
        reports[0].kernel.as_ref().unwrap().active_play.plan_id,
        reports[1].kernel.as_ref().unwrap().active_play.plan_id
    );
    for report in &reports {
        assert!(matches!(
            report.observations.last().map(|value| &value.kind),
            Some(ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        ));
        assert_eq!(count(report, KernelEventKind::HostCallRequested), 1);
        assert_eq!(count(report, KernelEventKind::HostCallCompleted), 1);
        assert_eq!(count(report, KernelEventKind::RunCancelled), 0);
        assert!(report.control_receipts.is_empty());
    }
    assert_eq!(outputs.0.len(), 2);
    assert_eq!(outputs.0[0], outputs.0[1]);
    // Aggregate adapter counters span two distinct Plays, never a merged Sign.
    assert_eq!(counts.executes.load(Ordering::SeqCst), 2);
    assert_eq!(counts.polls.load(Ordering::SeqCst), 4);
    assert_eq!(counts.ready.load(Ordering::SeqCst), 2);
    assert_eq!(counts.cancels.load(Ordering::SeqCst), 0);
    control
        .request_stop(RunControlRequestId::new("model-completed/late-stop").unwrap())
        .unwrap();
    // This later request cannot retroactively change either terminal report.
    assert_eq!(outputs.0.len(), 2);
    for report in &reports {
        assert_eq!(count(report, KernelEventKind::HostCallCompleted), 1);
    }
}

#[test]
fn pending_provider_loss_has_no_output_or_hidden_retry() {
    let (mut reports, outputs, counts, _) = run(Mode::ProviderLost, 1);
    // Public runner currently returns a String on BackFailed, not a typed
    // failure report; do not pretend this asserts an unavailable failure Sign.
    let error = reports.pop().unwrap().unwrap_err();
    assert!(
        error.starts_with("installed kernel step: BackFailed"),
        "{error}"
    );
    assert!(error.contains("code: HostCallFailed"), "{error}");
    assert!(error.contains("detail: 71"), "{error}");
    assert!(outputs.0.is_empty());
    assert_eq!(counts.executes.load(Ordering::SeqCst), 1);
    assert_eq!(counts.polls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.lost.load(Ordering::SeqCst), 1);
    assert_eq!(counts.ready.load(Ordering::SeqCst), 0);
}
