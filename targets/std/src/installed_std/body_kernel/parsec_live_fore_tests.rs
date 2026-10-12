//! Actual constructor advertisement and one issued retained Body Play.
//! Commands are typed software ingress; this does not prove installed keyboard binding.
use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, RunControl, StdHost,
    StdHostConfig, TimerAdapter,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{port_id, ConnectionTrack, HostAdvertisement, PortDirection};
use conduit_core::{BootId, HostId, OfferGeneration, SignId, TerminalDisposition};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{ParsecMapCommand, ParsecMapTransitionResult, PARSEC_MAP_FRAME_BYTES};
use std::collections::BTreeMap;
use std::{sync::mpsc, time::Duration};

#[path = "parsec_input_map_body_tests.rs"]
mod input_map;

struct NoTimer;

impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}

fn prepared() -> (
    StdHost,
    BodyPlan,
    conduit_body::Wake,
    BodyLiveForeQueue,
    RunControl,
) {
    let host = StdHost::new_for_parsec_scan(
        StdHostConfig {
            host_id: HostId::from("std-parsec-offer-proof"),
            boot_id: BootId::from("std-parsec-offer-proof/boot"),
            offer_generation: OfferGeneration(1),
        },
        32,
    )
    .unwrap();
    let plan = authored_plan(host.advertisement());
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_parsec_plan(&plan, control.clone(), 1).unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("sign/live-parsec-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/live-parsec-wake"))
        .unwrap()
        .1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    (host, body_plan, wake, queue, control)
}

fn authored_plan(advertisement: &HostAdvertisement) -> conduit_core::Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    crate::flow_activation::install_parsec_scan_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(crate::flow_activation::PARSEC_SCAN_SOURCE),
        &startup,
    )
    .unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "parsec/main", &profile).unwrap();
    let hosts = [advertisement.clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
            },
        ),
    ]);
    conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &expanded,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &bounds,
    )
    .unwrap()
}
fn command(action: &str, keyboard: bool, revision: u64) -> Vec<u8> {
    ParsecMapCommand::new(action.into(), revision, keyboard)
        .unwrap()
        .into_structured()
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

#[test]
fn parsec_commands_retain_source_map_in_one_actual_body_play() {
    let (mut host, plan, wake, queue, control) = prepared();
    let inputs = [
        command("game/start", false, 1),
        command("game/shoot", false, 2),
        command("game/remap", true, 2),
        command("game/shoot", false, 2),
        command("game/start", true, 3),
        command("game/shoot", false, 3),
    ];
    let (started_tx, started_rx) = mpsc::channel();
    let (delivered_tx, delivered_rx) = mpsc::channel();
    let producer = queue.clone();
    let sender = std::thread::spawn(move || {
        started_rx.recv().unwrap();
        for (sequence, input) in inputs.iter().enumerate() {
            assert_eq!(
                producer.submit(input).unwrap(),
                BodyLiveForeAdmission::Accepted {
                    sequence: sequence as u64
                }
            );
            delivered_rx.recv().unwrap();
        }
        producer.close().unwrap();
    });
    struct AcknowledgedCapture {
        values: Vec<ExternalForeDelivery>,
        delivered: mpsc::Sender<()>,
    }
    impl BodyForeOutputAdapter for AcknowledgedCapture {
        fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
            self.values.push(value.clone());
            self.delivered.send(()).map_err(|error| error.to_string())
        }
    }
    let mut capture = AcknowledgedCapture {
        values: Vec::new(),
        delivered: delivered_tx,
    };
    let report = host
        .run_body_plan_with_live_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &queue,
            &mut capture,
            &mut Vec::new(),
            &mut NoTimer,
            |play, _| {
                started_tx.send(play.active_play_id.clone()).unwrap();
                Ok(())
            },
        )
        .unwrap();
    sender.join().unwrap();
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert_eq!(report.play.play_sequence, 0);
    assert_eq!(capture.values.len(), 6);
    for (index, accepted, keyboard, revision) in [
        (0, true, false, 2),
        (1, true, false, 2),
        (2, true, true, 3),
        (3, false, true, 3),
        (4, false, true, 3),
        (5, true, true, 3),
    ] {
        let result = ParsecMapTransitionResult::from_structured(
            conduit_core::StructuredInfoValue::from_canonical_bytes(&capture.values[index].bytes)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(*result.accepted(), accepted);
        assert!(*result.state().started());
        assert_eq!(*result.state().keyboard(), keyboard);
        assert_eq!(*result.state().mapping_revision(), revision);
    }
    let status = report.live_fore_status.unwrap();
    assert_eq!(
        (status.queue_accepted, status.kernel_admitted, status.queued),
        (6, 6, 0)
    );
    assert!(status.close_requested && status.kernel_closed && status.play_terminal);
    let receipts = report.scan_child_signs.unwrap();
    assert_eq!(receipts.len(), 6);
    assert!(receipts.iter().all(|receipt| receipt.parent_active_play_id
        == report.play.active_play_id
        && !receipt.events.is_empty()));
}

#[test]
fn combined_parsec_pending_keyboard_progresses_without_a_command_then_cancels() {
    use crate::hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll};
    let mut host = BodyLiveForeQueue::parsec_input_test_host(StdHostConfig {
        host_id: "std/combined-input-proof".into(),
        boot_id: "std/combined-input-proof/boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let control = RunControl::default();
    let selected = BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    let queue = BodyLiveForeQueue::for_parsec_plan(&selected, control.clone(), 1).unwrap();
    let body = Body::born(
        selected.source_document_id.clone(),
        selected.checked_plot_id.clone(),
        1,
        SignId::from("sign/combined-input-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/combined-input-wake"))
        .unwrap()
        .1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                selected.source_document_id.clone(),
                selected.checked_plot_id.clone(),
            ),
            plan: selected,
        }],
    )
    .unwrap();
    struct PendingThenSpace {
        polls: usize,
    }
    impl HostedKeyboardAdapter for PendingThenSpace {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            self.polls += 1;
            if self.polls == 5 {
                HostedKeyboardPoll::Event(conduit_human::KeyEvent::decode(&[0x2c, 0, 0]).unwrap())
            } else {
                HostedKeyboardPoll::Pending
            }
        }
    }
    struct Capture {
        values: Vec<ExternalForeDelivery>,
        control: RunControl,
    }
    impl BodyForeOutputAdapter for Capture {
        fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
            self.values.push(value.clone());
            if self.values.len() == 2 {
                self.control
                    .request_stop(crate::RunControlRequestId::new("both-inputs-observed").unwrap())
                    .map_err(|e| format!("stop: {e:?}"))?;
            }
            Ok(())
        }
    }
    // Bound the regression: a queue wait that suppresses pending keyboard
    // progress must stop and fail assertions rather than hang the test suite.
    let (done_tx, done_rx) = mpsc::channel();
    let (start_tx, start_rx) = mpsc::channel();
    let stop = control.clone();
    let guard = std::thread::spawn(move || {
        if start_rx.recv().is_err() {
            return;
        }
        if done_rx.recv_timeout(Duration::from_secs(2)).is_err() {
            let _ = stop
                .request_stop(crate::RunControlRequestId::new("input-progress-timeout").unwrap());
        }
    });
    let mut keyboard = PendingThenSpace { polls: 0 };
    let mut capture = Capture {
        values: Vec::new(),
        control: control.clone(),
    };
    let report = host.run_body_plan_with_live_fore_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: Some(&mut keyboard),
        },
        &queue,
        &mut capture,
        &mut Vec::new(),
        &mut NoTimer,
        |_, _| start_tx.send(()).map_err(|e| e.to_string()),
    );
    drop(start_tx);
    let _ = done_tx.send(());
    guard.join().unwrap();
    let report = report.unwrap();
    assert!(
        keyboard.polls >= 5,
        "polls={} terminal={:?} failure={:?} kernel={:?} cleanup={:?}",
        keyboard.polls,
        report.terminal,
        report.failure,
        report.kernel_failure,
        report.cleanup_failure
    );
    assert_eq!(
        capture.values.len(),
        2,
        "terminal={:?} failure={:?} kernel={:?} cleanup={:?}",
        report.terminal,
        report.failure,
        report.kernel_failure,
        report.cleanup_failure
    );
    let key = capture
        .values
        .iter()
        .find(|v| v.front_port_id.as_str() == "keys")
        .unwrap();
    assert_eq!(key.bytes, [0x2c, 0, 0]);
    let fire = capture
        .values
        .iter()
        .find(|v| v.front_port_id.as_str() == "fire")
        .unwrap();
    let transition = conduit_core::StructuredInfoValue::from_canonical_bytes(&fire.bytes).unwrap();
    assert_eq!(
        transition.value_type(),
        &conduit_semantic_catalog::input_button_transition_type()
    );
    assert_eq!(report.live_fore_status.unwrap().queue_accepted, 0);
    assert!(queue.status().play_terminal);
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert!(queue.submit(&command("game/start", false, 1)).is_err());
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert!(
        report.cleanup_failure.is_none(),
        "{:?}",
        report.cleanup_failure
    );
}

#[path = "parsec_input_action_body_tests.rs"]
mod input_actions;
