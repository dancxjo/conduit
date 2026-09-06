use conduit_core::{CancellationReason, ObservationKind, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{
    RunControl, RunControlDisposition, RunControlRequestId, StdHost, TimerAdapter,
};
use std::time::Duration;

struct StopOnFirstWait {
    control: RunControl,
    waits: usize,
}

impl TimerAdapter for StopOnFirstWait {
    fn wait(&mut self, _duration: Duration) {
        self.waits += 1;
        self.control
            .request_stop(RunControlRequestId::new("patchbay/stop-1").unwrap())
            .expect("the first exact Stop request is admitted");
    }
}

#[test]
fn exact_stop_request_uses_scheduler_cancellation_and_returns_terminal_sign() {
    let source = include_str!("../../../forms/clock/main.conduit");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "clock-demo", &profile).unwrap();
    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let control = RunControl::default();
    let mut timer = StopOnFirstWait {
        control: control.clone(),
        waits: 0,
    };
    let mut output = Vec::with_capacity(256);

    let report = host
        .run_fragment_controlled_to(plan.fragments[0].clone(), &mut output, &mut timer, &control)
        .unwrap();

    assert_eq!(timer.waits, 1);
    let rendered = String::from_utf8(output.clone()).unwrap();
    assert!(rendered.contains(" cancelled reason=OperatorRequested\n"));
    assert!(!rendered.contains(" complete\n"));
    assert!(matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled {
                reason: CancellationReason::OperatorRequested
            }
        })
    ));
    assert_eq!(report.control_receipts.len(), 1);
    let sign = &report.kernel.as_ref().unwrap().kernel_sign;
    assert!(sign
        .iter()
        .any(|event| { event.kind == conduit_kernel::KernelEventKind::CancellationRequested }));
    assert!(sign
        .iter()
        .any(|event| { event.kind == conduit_kernel::KernelEventKind::RunCancelled }));
    assert_eq!(
        report.control_receipts[0].request_id.as_str(),
        "patchbay/stop-1"
    );
    assert_eq!(
        report.control_receipts[0].disposition,
        RunControlDisposition::Accepted
    );
    assert_eq!(
        report.control_receipts[0].active_play_id,
        report.kernel.as_ref().unwrap().active_play_id
    );
    assert!(output
        .windows("tick sequence=0".len())
        .all(|window| window != b"tick sequence=0"));
}

#[test]
fn duplicate_stop_request_is_rejected_with_its_own_identity() {
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("stop/first").unwrap())
        .unwrap();
    let rejected = control
        .request_stop(RunControlRequestId::new("stop/duplicate").unwrap())
        .unwrap_err();
    assert_eq!(rejected.request_id.as_str(), "stop/duplicate");
    assert_eq!(
        rejected.disposition,
        RunControlDisposition::RejectedAlreadyRequested
    );
}
