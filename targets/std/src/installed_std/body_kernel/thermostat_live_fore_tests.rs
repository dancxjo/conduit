use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, RunControl, StdHost,
    StdHostConfig, TimerAdapter,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{BootId, HostId, OfferGeneration, SignId, TerminalDisposition};
use conduit_thermostat_plot::{Command, Fan, Mode, ThermostatState};
use std::{sync::mpsc, time::Duration};

struct NoTimer;

impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}

struct Capture {
    values: Vec<ExternalForeDelivery>,
    first: Option<mpsc::Sender<()>>,
}

impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        self.values.push(value.clone());
        if let Some(sender) = self.first.take() {
            sender.send(()).map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

fn prepared() -> (
    StdHost,
    BodyPlan,
    conduit_body::Wake,
    BodyLiveForeQueue,
    RunControl,
) {
    let initial = ThermostatState::default();
    let host = StdHost::new_for_thermostat_scan(
        StdHostConfig {
            host_id: HostId::from("std-thermostat-offer-proof"),
            boot_id: BootId::from("std-thermostat-offer-proof/boot"),
            offer_generation: OfferGeneration(1),
        },
        &initial,
        2,
    )
    .unwrap();
    let plan = crate::flow_activation::authored_thermostat_plan_on_host(host.advertisement(), 2);
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_thermostat_plan(&plan, control.clone(), 1).unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("sign/live-thermostat-born"),
    )
    .unwrap();
    let wake = body
        .wake(1, SignId::from("sign/live-thermostat-wake"))
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

#[test]
fn thermostat_controls_retain_accumulator_in_one_native_body_play() {
    let (mut host, plan, wake, queue, control) = prepared();
    let first = Command::SetMode(Mode::Heat).encode();
    let second = Command::SetFan(Fan::On).encode();
    let (started_tx, started_rx) = mpsc::channel();
    let (first_tx, first_rx) = mpsc::channel();
    let producer = queue.clone();
    let sender = std::thread::spawn(move || {
        started_rx.recv().unwrap();
        assert_eq!(
            producer.submit(&first).unwrap(),
            BodyLiveForeAdmission::Accepted { sequence: 0 }
        );
        first_rx.recv().unwrap();
        assert_eq!(
            producer.submit(&second).unwrap(),
            BodyLiveForeAdmission::Accepted { sequence: 1 }
        );
        producer.close().unwrap();
    });
    let mut capture = Capture {
        values: Vec::new(),
        first: Some(first_tx),
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
    assert_eq!(capture.values.len(), 2);
    assert_eq!(report.play.play_sequence, 0);
    let first_state = ThermostatState::decode(&capture.values[0].bytes).unwrap();
    let second_state = ThermostatState::decode(&capture.values[1].bytes).unwrap();
    assert_eq!(first_state.mode, Mode::Heat);
    assert_eq!(second_state.mode, Mode::Heat);
    assert_eq!(second_state.fan, Fan::On);
    assert_eq!((first_state.revision, second_state.revision), (1, 2));

    let status = report.live_fore_status.unwrap();
    assert_eq!(
        (status.queue_accepted, status.kernel_admitted, status.queued),
        (2, 2, 0)
    );
    assert!(status.close_requested && status.kernel_closed && status.play_terminal);
    let receipts = report.scan_child_signs.unwrap();
    assert_eq!(receipts.len(), 2);
    assert!(receipts
        .iter()
        .all(|receipt| receipt.parent_active_play_id == report.play.active_play_id));
}
