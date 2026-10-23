//! Actual configured native worker lifecycle through authored Source/Plan/Play.
//! SIGSTOP proves pending cancellation, not interruption of native CPU compute.
#![cfg(all(
    feature = "native-model-backing",
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu"
))]
#[path = "common_model_infer/mod.rs"]
mod common;
#[allow(dead_code)] // Shared fixture also serves backing/reference controls in its owning suite.
#[path = "native_model_infer/fixture.rs"]
mod fixture;
#[path = "native_model_infer/lifecycle_process.rs"]
mod process;
use conduit_ai::{
    imported_back_admission::ImportedBackAdmissionReceipt,
    imported_model_descriptor::ImportedModelInspection,
};
use conduit_core::{CancellationReason, ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_kernel::KernelEventKind;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    hosted_native_model::prepare_native_model, ExternalForeInput, RunControl,
    RunControlDisposition, RunControlRequestId, StdHost, StdRunReport,
};
use std::time::{Duration, Instant};
fn prepared() -> (
    StdHost,
    conduit_core::Plan,
    std::sync::Arc<process::Worker>,
    ImportedBackAdmissionReceipt,
) {
    let before = process::children();
    let mut admission = None;
    let (host, plan) = common::host_plan(fixture::SOURCE, |signature| {
        let prepared = prepare_native_model(
            fixture::model(signature),
            fixture::worker(),
            fixture::profile(),
            fixture::grant(),
        )
        .unwrap();
        admission = Some(prepared.imported_admission().unwrap().clone());
        Box::new(prepared)
    });
    let worker = process::Worker::discover(&before);
    assert_eq!(
        std::fs::read_link(format!("/proc/{}/exe", worker.pid)).unwrap(),
        std::fs::canonicalize(fixture::worker()).unwrap()
    );
    let admission = admission.unwrap();
    let inspection =
        ImportedModelInspection::from_planned_gear(&plan.fragments[0].placements[0]).unwrap();
    assert_eq!(inspection.admission_identity, admission.identity());
    assert_eq!(
        plan.fragments[0].placements[0].host_calls[0]
            .contract_id
            .as_str(),
        conduit_std_host::hosted_model_infer::MODEL_INFER_OPERATION
    );
    (host, plan, worker, admission)
}
fn input(bits: u32) -> [ExternalForeInput; 1] {
    [ExternalForeInput {
        front_port_id: "inputs".into(),
        track: ConnectionTrack::Payload,
        bytes: fixture::scalar(bits).encode().unwrap(),
    }]
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
#[test]
#[ignore = "requires explicit installed ORT; actual native lifecycle proof"]
fn same_prepared_native_worker_serves_two_ordinary_plays_then_host_drop_reaps() {
    let (mut host, plan, worker, admission) = prepared();
    let children = process::children();
    let maps = worker.maps();
    let mut plays = Vec::new();
    for bits in [0x8000_0000, 0x7fc1_2345] {
        let mut output = common::Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &input(bits),
                &mut output,
                &mut Vec::new(),
                &mut common::NoTimer,
            )
            .unwrap();
        assert!(matches!(
            report.observations.last().map(|value| &value.kind),
            Some(ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        ));
        assert_eq!(output.0, vec![fixture::scalar(bits)]);
        assert_eq!(count(&report, KernelEventKind::HostCallRequested), 1);
        assert_eq!(count(&report, KernelEventKind::HostCallCompleted), 1);
        let kernel = report.kernel.unwrap();
        assert_eq!(kernel.active_play.plan_id, plan.plan_id);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        plays.push(kernel.active_play_id);
        assert!(worker.alive());
        assert_eq!(worker.maps(), maps);
        assert_eq!(process::children(), children);
        assert_eq!(
            ImportedModelInspection::from_planned_gear(&plan.fragments[0].placements[0])
                .unwrap()
                .admission_identity,
            admission.identity()
        );
    }
    assert_eq!(plays.len(), 2); // Each report binds its own ordinary Play; same exact Plan/owner.
    drop(host);
    worker
        .wait_gone(Instant::now() + Duration::from_secs(2))
        .unwrap();
    println!(
        "{}",
        serde_json::json!({"proof":"native-warm-owner-lifecycle","pid":worker.pid,"starttime":worker.starttime,"source":plan.source_document_id.as_str(),"plan":plan.plan_id.as_str(),"plays":plays.iter().map(|play|play.as_str()).collect::<Vec<_>>(),"admission":admission.identity(),"maps_unchanged":true,"child_reaped":true,"not_established":["cross-artifact/profile cache reuse","native execution latency"]})
    );
}
#[test]
#[ignore = "requires explicit installed ORT; actual native lifecycle proof"]
fn ordinary_stop_cancels_actual_worker_with_nine_request_bytes_queued() {
    let (mut host, plan, worker, _) = prepared();
    let children = process::children();
    let pipe = worker.request_observer();
    // Preparation returns after sending G; ensure it was consumed before STOP
    // so this barrier observes only the actual nine-byte inference request.
    worker.wait_empty(&pipe, Instant::now() + Duration::from_secs(2));
    worker.stop();
    let control = RunControl::default();
    let controller_control = control.clone();
    let controller_worker = worker.clone();
    let mut outputs = common::Outputs::default();
    let started = Instant::now();
    let (report, barrier) = std::thread::scope(|scope| {
        let controller = scope.spawn(move || {
            let result = (|| {
                let bytes = controller_worker
                    .wait_request(&pipe, Instant::now() + Duration::from_secs(10))?;
                controller_control
                    .request_stop(RunControlRequestId::new("native-worker/pending-stop").unwrap())
                    .map_err(|error| format!("{error:?}"))?;
                controller_worker.wait_gone(Instant::now() + Duration::from_secs(5))?;
                Ok::<i32, String>(bytes)
            })();
            if result.is_err() {
                let _ = controller_control.request_stop(
                    RunControlRequestId::new("native-worker/barrier-failed").unwrap(),
                );
                controller_worker.cleanup();
            }
            result
        });
        // Value Fore remains Value: Root's thin controlled ordinary-Play API.
        let report = host.run_external_plot_controlled_to(
            plan.fragments[0].clone(),
            &input(0x3fa0_0000),
            &mut outputs,
            &mut Vec::new(),
            &mut common::NoTimer,
            &control,
        );
        (report, controller.join().expect("controller panicked"))
    });
    assert_eq!(barrier.unwrap(), 9);
    let report = report.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "test responsiveness safety bound"
    );
    assert!(outputs.0.is_empty());
    assert!(!worker.alive());
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
        "native-worker/pending-stop"
    );
    assert_eq!(count(&report, KernelEventKind::HostCallRequested), 1);
    assert_eq!(
        count(&report, KernelEventKind::HostCallCancellationRequested),
        1
    );
    assert_eq!(count(&report, KernelEventKind::HostCallCompleted), 0);
    assert_eq!(
        report.control_receipts[0].active_play_id,
        report.kernel.as_ref().unwrap().active_play_id
    );
    let sign = &report.kernel.as_ref().unwrap().kernel_sign;
    let requested = sign
        .iter()
        .find(|event| event.kind == KernelEventKind::HostCallRequested)
        .unwrap();
    let cancelled = sign
        .iter()
        .find(|event| event.kind == KernelEventKind::HostCallCancellationRequested)
        .unwrap();
    assert_eq!(
        (requested.node, requested.request),
        (cancelled.node, cancelled.request)
    );
    let mut remaining = children;
    remaining.remove(&worker.pid);
    assert_eq!(process::children(), remaining);
    drop(host);
    println!(
        "{}",
        serde_json::json!({"proof":"actual-native-pending-stop","pid":worker.pid,"starttime":worker.starttime,"request_bytes_observed_without_read":9,"cancelled":true,"outputs":0,"child_reaped":true,"not_established":["interruption during native CPU computation","native execution performance"]})
    );
}
#[test]
#[ignore = "requires explicit installed ORT; actual native lifecycle proof"]
fn killed_actual_provider_cannot_publish_or_spawn_a_retry() {
    let (mut host, plan, worker, _) = prepared();
    let mut expected = process::children();
    expected.remove(&worker.pid);
    worker.signal(libc::SIGKILL).unwrap();
    worker.wait_exited(Instant::now() + Duration::from_secs(2));
    let mut outputs = common::Outputs::default();
    let result = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &input(0x3fa0_0000),
        &mut outputs,
        &mut Vec::new(),
        &mut common::NoTimer,
    );
    let error = result.unwrap_err();
    assert!(
        error.starts_with("installed kernel step: BackFailed"),
        "{error}"
    );
    assert!(error.contains("code: HostCallFailed"), "{error}");
    assert!(error.contains("detail: 71"), "{error}");
    assert!(outputs.0.is_empty());
    worker
        .wait_gone(Instant::now() + Duration::from_secs(2))
        .unwrap();
    assert_eq!(process::children(), expected);
    drop(host);
    println!(
        "{}",
        serde_json::json!({"proof":"actual-native-provider-loss","pid":worker.pid,"starttime":worker.starttime,"error_boundary":"existing String report","error":error,"outputs":0,"child_reaped":true,"retry_child":false})
    );
}
