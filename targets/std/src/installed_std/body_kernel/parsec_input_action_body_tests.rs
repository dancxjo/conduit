//! Actual software input -> concrete terminal return -> checked source command.
use super::*;
use crate::body_execution::{BodyParsecShowObserver, BodyParsecTerminalRoute};
use crate::hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll};
use conduit_presentation::{MaskShow, Presentation};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[test]
fn combined_body_admits_start_shoot_and_remapped_enter_from_completed_input() {
    let baseline = BodyLiveForeQueue::parsec_input_test_host(StdHostConfig {
        host_id: "std/combined-map-input-proof".into(),
        boot_id: "std/combined-map-input-proof/boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut advertisement = baseline.advertisement().clone();
    for offer in crate::terminal_mask_execution::planning::attached_terminal_capabilities() {
        if let Some(existing) = advertisement
            .capabilities
            .iter()
            .find(|old| old.capability_id == offer.capability_id)
        {
            assert_eq!(existing, &offer);
        } else {
            advertisement.capabilities.push(offer);
        }
    }
    for resource in crate::terminal_mask_execution::planning::attached_terminal_resources() {
        if !advertisement.resources.contains(&resource) {
            advertisement.resources.push(resource);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement.resources.sort();
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let selected = BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_parsec_plan(&selected, control.clone(), 1).unwrap();
    let body = Body::born(
        selected.source_document_id.clone(),
        selected.checked_plot_id.clone(),
        1,
        SignId::from("sign/combined-map-input-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/combined-map-input-wake"))
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

    let states = Arc::new(AtomicUsize::new(0));
    let fires = Arc::new(AtomicUsize::new(0));
    struct Keys {
        states: Arc<AtomicUsize>,
        fires: Arc<AtomicUsize>,
        next: usize,
    }
    impl HostedKeyboardAdapter for Keys {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            let states = self.states.load(Ordering::SeqCst);
            let fires = self.fires.load(Ordering::SeqCst);
            let ready = match self.next {
                0 => true,
                1 => states >= 1,
                2 => states >= 1 && fires >= 2,
                3 => states >= 2,
                4 => states >= 3 && fires >= 4,
                5 => fires >= 5,
                6 => fires >= 6,
                _ => false,
            };
            if !ready {
                return HostedKeyboardPoll::Pending;
            }
            let bytes = match self.next {
                1 | 3 | 5 => [0x2c, 1, 0],
                6 => [0x28, 0, 0],
                _ => [0x2c, 0, 0],
            };
            self.next += 1;
            HostedKeyboardPoll::Event(conduit_human::KeyEvent::decode(&bytes).unwrap())
        }
    }
    struct Capture {
        values: Vec<ExternalForeDelivery>,
        states: Arc<AtomicUsize>,
        fires: Arc<AtomicUsize>,
        queue: BodyLiveForeQueue,
        control: RunControl,
    }
    impl BodyForeOutputAdapter for Capture {
        fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
            self.values.push(value.clone());
            if value.front_port_id.as_str() == "fire" {
                self.fires.fetch_add(1, Ordering::SeqCst);
            }
            if value.front_port_id.as_str() == "states" {
                let count = self.states.fetch_add(1, Ordering::SeqCst) + 1;
                if count == 2 {
                    let admitted = self.queue.submit(&command("game/remap", true, 2))?;
                    if admitted != (BodyLiveForeAdmission::Accepted { sequence: 2 }) {
                        return Err(format!("remap command staging: {admitted:?}"));
                    }
                }
                if count == 4 {
                    self.control
                        .request_stop(
                            crate::RunControlRequestId::new("input-actions-proved").unwrap(),
                        )
                        .map_err(|error| format!("stop: {error:?}"))?;
                }
            }
            Ok(())
        }
    }
    #[derive(Default)]
    struct Shows(Vec<(Presentation, MaskShow)>);
    impl BodyParsecShowObserver for Shows {
        fn showed(&mut self, face: &Presentation, show: &MaskShow) -> Result<(), String> {
            show.validate(face)
                .map_err(|error| format!("Show: {error:?}"))?;
            self.0.push((face.clone(), show.clone()));
            Ok(())
        }
    }
    let (watch_tx, watch_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let guard_control = control.clone();
    let guard = std::thread::spawn(move || {
        if watch_rx.recv().is_ok() && done_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            let _ = guard_control
                .request_stop(crate::RunControlRequestId::new("input-action-watchdog").unwrap());
        }
    });
    let mut keys = Keys {
        states: states.clone(),
        fires: fires.clone(),
        next: 0,
    };
    let mut capture = Capture {
        values: Vec::new(),
        states,
        fires,
        queue: queue.clone(),
        control: control.clone(),
    };
    let mut shows = Shows::default();
    let mut terminal = Vec::new();
    let result = host.run_body_plan_with_parsec_terminal_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: Some(&mut keys),
        },
        BodyParsecTerminalRoute {
            queue: &queue,
            fore_output: &mut capture,
            terminal_output: &mut terminal,
            observer: &mut shows,
            columns: 80,
            rows: 24,
        },
        &mut Vec::new(),
        &mut NoTimer,
        |_, _| watch_tx.send(()).map_err(|error| error.to_string()),
    );
    drop(watch_tx);
    let _ = done_tx.send(());
    guard.join().unwrap();
    let report = result.unwrap();
    assert_eq!(
        (
            report.failure.as_ref(),
            report.kernel_failure.as_ref(),
            report.cleanup_failure.as_ref()
        ),
        (None, None, None)
    );
    assert_eq!(
        keys.next, 7,
        "three Space presses including remapped refusal, then Enter"
    );
    let results: Vec<_> = capture
        .values
        .iter()
        .filter(|value| value.front_port_id.as_str() == "states")
        .map(|value| {
            ParsecMapTransitionResult::from_structured(
                conduit_core::StructuredInfoValue::from_canonical_bytes(&value.bytes).unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        results.len(),
        4,
        "Start, Shoot, remap and Enter; remapped Space has no command effect"
    );
    for (i, keyboard, revision) in [(0, false, 2), (1, false, 2), (2, true, 3), (3, true, 3)] {
        assert!(*results[i].accepted());
        assert!(*results[i].state().started());
        assert_eq!(*results[i].state().keyboard(), keyboard);
        assert_eq!(*results[i].state().mapping_revision(), revision);
    }
    assert!(shows.0.len() >= 3);
    assert_eq!(shows.0[0].0.actions[0].identity, "game/start");
    assert!(shows.0[1..]
        .iter()
        .all(|(face, _)| face.actions[0].identity == "game/shoot"));
    assert!(!terminal.is_empty());
    let status = report.live_fore_status.unwrap();
    assert_eq!(
        (status.queue_accepted, status.kernel_admitted, status.queued),
        (4, 4, 0)
    );
    assert_eq!(report.scan_child_signs.unwrap().len(), 4);
}
