//! Truthful failure, refusal, and cancellation of the basic episode loop in one
//! planned kernel Play. The model provider is the fenced deterministic fake.

#[path = "common_episode_loop/mod.rs"]
mod common;

use common::*;
use conduit_core::TerminalDisposition;
use conduit_std_host::hosted_local_model::LocalModelAdapterTerminal;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeOutputAdapter, RunControl, RunControlRequestId,
};

#[test]
fn provider_failure_is_terminal_and_fabricates_no_episode() {
    for (terminal_kind, code, detail) in [
        (
            LocalModelAdapterTerminal::Failed,
            "HostCallFailed",
            "detail: 53",
        ),
        (
            LocalModelAdapterTerminal::ProviderLost,
            "HostCallFailed",
            "detail: 54",
        ),
        (
            LocalModelAdapterTerminal::InvalidStructuredResult,
            "InvalidInput",
            "detail: 55",
        ),
    ] {
        let mut harness = harness(Some((2, terminal_kind)));
        let mut collector = Collector::default();
        let error = run(
            &mut harness,
            &inputs(&[("obs/1", "First.", true), ("obs/2", "Second.", true)]),
            &mut collector,
            &RunControl::default(),
        )
        .unwrap_err();
        assert!(error.contains("logical-gear=episode-loop/model"), "{error}");
        assert!(error.contains(code), "{error}");
        assert!(error.contains(detail), "{error}");
        let episodes = records(&collector.0);
        assert_eq!(episodes.len(), 1, "only the produced episode is delivered");
        assert_eq!(*episodes[0].episode(), 1);
        assert_eq!(harness.requests.lock().unwrap().len(), 2);
    }
}

#[test]
fn missing_context_fails_typed_without_calling_the_model() {
    let mut harness = harness(None);
    let mut collector = Collector::default();
    let error = run(
        &mut harness,
        &[observation("obs/1", "No body context.", true)],
        &mut collector,
        &RunControl::default(),
    )
    .unwrap_err();
    assert!(
        error.contains("InvalidInput") && error.contains("detail: 1"),
        "{error}"
    );
    assert!(collector.0.is_empty());
    assert!(harness.requests.lock().unwrap().is_empty());
}

#[test]
fn operator_cancellation_after_first_episode_is_truthful() {
    struct StopAfterFirst {
        control: RunControl,
        delivered: Vec<ExternalForeDelivery>,
    }
    impl ExternalForeOutputAdapter for StopAfterFirst {
        fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
            self.delivered.push(output);
            if self.delivered.len() == 1 {
                self.control
                    .request_stop(RunControlRequestId::new("stop-after-first-episode").unwrap())
                    .map_err(|error| format!("{error:?}"))?;
            }
            Ok(())
        }
    }
    let mut harness = harness(None);
    let control = RunControl::default();
    let mut outputs = StopAfterFirst {
        control: control.clone(),
        delivered: Vec::new(),
    };
    let report = run(
        &mut harness,
        &inputs(&[
            ("obs/1", "One.", true),
            ("obs/2", "Two.", true),
            ("obs/3", "Three.", true),
        ]),
        &mut outputs,
        &control,
    )
    .unwrap();
    assert!(matches!(
        terminal(&report),
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(report.control_receipts.len(), 1);
    assert_eq!(
        report.control_receipts[0].request_id.as_str(),
        "stop-after-first-episode"
    );
    let episodes = records(&outputs.delivered);
    assert_eq!(episodes.len(), 1);
    assert!(harness.requests.lock().unwrap().len() < 3);
}
