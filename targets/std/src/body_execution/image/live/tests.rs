//! Actual authored closing Flow fed after Play start through the existing queue.
use super::*;
use crate::body_execution::{
    CommittedFaceContribution, FaceContributionRefusal, ReturnedFaceContribution,
};
use crate::{ExternalForeDelivery, ExternalForeInput, RunControlRequestId, ThreadTimer};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{ConnectionTrack, HostAdvertisement, Plan, PortDirection, StructuredInfoValue};
use conduit_data::*;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use std::{
    collections::BTreeMap,
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};
const SOURCE: &str = concat!(
    include_str!("../../../../../../plots/measurement-inspection/main.conduit"),
    r#"
plot inspection/charts (
    >> chart: MeasurementChart...|
) {
    view: measurement-inspection("Conditions", 2)
    chart >> view.chart
}"#,
);
fn planned(host: &HostAdvertisement) -> (BodyPlan, Wake) {
    planned_source(host, SOURCE)
}
fn planned_for_update_bound(host: &HostAdvertisement, maximum: u32) -> (BodyPlan, Wake) {
    assert!((1..=4096).contains(&maximum));
    planned_source(
        host,
        &SOURCE.replace(
            "measurement-inspection(\"Conditions\", 2)",
            &format!("measurement-inspection(\"Conditions\", {maximum})"),
        ),
    )
}
fn planned_source(host: &HostAdvertisement, source: &str) -> (BodyPlan, Wake) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profiles)
        .unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profiles).unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "inspection/charts", &profiles)
            .unwrap();
    let hosts = [host.clone()];
    let selected =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundary = BTreeMap::from([(
        ForeBoundaryKey {
            direction: PortDirection::Input,
            front_port_id: "chart".into(),
            track: ConnectionTrack::Payload,
        },
        ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: measurement_chart_canonical_bytes() as u32,
        },
    )]);
    let plan: Plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &selected,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: measurement_chart_canonical_bytes() as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary,
    )
    .unwrap();
    let plot = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "proof/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "proof/wake".into()).unwrap().1;
    (
        BodyPlan::seal(&wake, vec![BodyPlotPlan { plot, plan }]).unwrap(),
        wake,
    )
}
fn input(value: i64) -> ExternalForeInput {
    let entries = ["North", "South", "East", "West"]
        .into_iter()
        .map(|label| {
            let series = MeasurementPlotSeries::from_projected(
                vec![
                    MeasurementPlotPoint::new(0, 0, value).unwrap(),
                    MeasurementPlotPoint::new(1, 1_000_000, value + 1).unwrap(),
                ],
                2,
                0,
            )
            .unwrap();
            LabelledMeasurementSeries::new(label.into(), series).unwrap()
        })
        .collect::<Vec<_>>();
    let bytes = StructuredInfoValue::leaf(
        measurement_chart_type(),
        encode_measurement_chart(&MeasurementChart::new(entries).unwrap()).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    ExternalForeInput {
        front_port_id: "chart".into(),
        track: ConnectionTrack::Payload,
        bytes,
    }
}

struct Outputs;
impl BodyForeOutputAdapter for Outputs {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        Err("no Fore output declared".into())
    }
}
struct Capture {
    frames: Vec<CommittedFaceContribution>,
    first: Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>,
    pressure: usize,
}
impl BodyFaceContributionObserver for Capture {
    fn contribute(
        &mut self,
        frame: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        if let Some((notify, ready)) = self.first.take() {
            notify.send(()).unwrap();
            ready
                .recv_timeout(Duration::from_secs(5))
                .expect("bounded producer queues second and proves Full");
        }
        if self.pressure > 0 {
            self.pressure -= 1;
            return Err(ReturnedFaceContribution {
                contribution: frame,
                refusal: FaceContributionRefusal::Pressure,
            });
        }
        assert!(self.frames.len() < self.frames.capacity());
        self.frames.push(frame);
        Ok(())
    }
}
#[test]
fn actual_live_closing_flow_keeps_original_queue_charge_and_commits_distinct_frames() {
    let mut host = crate::image_initialization::cpu_image_test_fixture(64 * 1024 * 1024);
    let (plan, wake) = planned_for_update_bound(host.advertisement(), 3);
    let control = RunControl::default();
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    // These fixture inputs were constructed under separate test-owner custody;
    // retained payload charges move with the producer, independently of queue.
    let inputs = [input(100), input(200), input(300)]
        .into_iter()
        .map(|input| host.retain_selected_image_fore_input(input).unwrap())
        .collect::<Vec<_>>();
    // SAFETY: this one reviewed fixture's complete construction/Plan validation
    // peak is independently covered by this conservative 8MiB test envelope.
    let queue = unsafe {
        host.prepare_selected_image_live_fore_queue(
            8 * 1024 * 1024,
            &wake,
            &plan,
            control.clone(),
            "chart",
            1,
            3,
        )
    }
    .unwrap();
    let charged = observed.load(Ordering::Acquire);
    let survivor = queue.clone();
    assert_eq!(
        observed.load(Ordering::Acquire),
        charged,
        "clone must not create another charge"
    );
    assert!(queue.submit(&inputs[0].input().bytes).is_err());
    let (first_tx, first_rx) = mpsc::sync_channel(1);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let mut capture = Capture {
        frames: Vec::with_capacity(3),
        first: Some((first_tx, ready_rx)),
        pressure: 2,
    };
    let producer = queue.clone();
    let producer_control = control.clone();
    let thread = std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            producer.wait_until_play_started()?;
            if producer.submit(&inputs[0].input().bytes)?
                != (BodyLiveForeAdmission::Accepted { sequence: 0 })
            {
                return Err("first live input was not accepted exactly".into());
            }
            first_rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?;
            if producer.submit(&inputs[1].input().bytes)?
                != (BodyLiveForeAdmission::Accepted { sequence: 1 })
            {
                return Err("second live input was not accepted exactly".into());
            }
            if producer.submit(&inputs[2].input().bytes)? != BodyLiveForeAdmission::Full {
                return Err("live staging did not preserve its one-slot Full boundary".into());
            }
            ready_tx.send(()).map_err(|e| e.to_string())?;
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                match producer.submit(&inputs[2].input().bytes)? {
                    BodyLiveForeAdmission::Accepted { sequence: 2 } => break,
                    BodyLiveForeAdmission::Full if Instant::now() < until => {
                        std::thread::yield_now()
                    }
                    _ => {
                        return Err("bounded live producer did not retain exact third input".into())
                    }
                }
            }
            producer.close()
        })();
        if result.is_err() {
            let _ = producer_control
                .request_stop(RunControlRequestId::new("test/live-producer-failed").unwrap());
        }
        result
    });
    let report = host
        .run_selected_image_body_plan_with_live_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &queue,
            &mut Outputs,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| Ok(()),
        )
        .unwrap();
    thread.join().unwrap().unwrap();
    assert_eq!(
        report.report().terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert!(
        report.report().failure.is_none(),
        "{:?}",
        report.report().failure
    );
    assert_eq!(capture.frames.len(), 3);
    for (index, frame) in capture.frames.iter().enumerate() {
        assert_eq!(frame.request(), conduit_kernel::RequestId(index as u32));
        assert_eq!(frame.scope().body_play(), &report.report().play);
        assert_eq!(frame.scope().host(), &host.advertisement().host_id);
        assert_eq!(frame.scope().boot(), &host.advertisement().boot_id);
    }
    assert_ne!(
        capture.frames[0].fragment().properties,
        capture.frames[1].fragment().properties
    );
    assert_eq!(queue.status().kernel_admitted, 3);
    assert!(queue.status().kernel_closed && queue.status().play_terminal);
    drop(report);
    drop(capture);
    drop(queue);
    assert!(
        observed.load(Ordering::Acquire) > baseline,
        "last producer handle retains original queue charge"
    );
    drop(survivor);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
}
#[test]
fn foreign_counter_and_preparation_pressure_refuse_without_start_or_queue_growth() {
    let mut host = crate::image_initialization::cpu_image_test_fixture(64 * 1024 * 1024);
    let (plan, wake) = planned(host.advertisement());
    let control = RunControl::default();
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    // SAFETY: reservation is deliberately impossible; constructor must not run.
    assert!(unsafe {
        host.prepare_selected_image_live_fore_queue(
            128 * 1024 * 1024,
            &wake,
            &plan,
            control.clone(),
            "chart",
            1,
            2,
        )
    }
    .is_err());
    assert_eq!(observed.load(Ordering::Acquire), baseline);
    // SAFETY: independently qualified finite fixture construction envelope.
    let queue = unsafe {
        host.prepare_selected_image_live_fore_queue(
            8 * 1024 * 1024,
            &wake,
            &plan,
            control.clone(),
            "chart",
            1,
            2,
        )
    }
    .unwrap();
    let foreign = crate::image_initialization::cpu_image_test_fixture(64 * 1024 * 1024);
    let foreign_play = foreign.prospective_body_play(&wake, &plan).unwrap();
    assert!(
        queue
            .queue
            .verify_image_admission(&foreign, &foreign_play, &control)
            .is_err(),
        "equal IMAGE/Host labels cannot transfer original counter"
    );
    let wrong_control = RunControl::default();
    let play = host.prospective_body_play(&wake, &plan).unwrap();
    assert!(queue
        .queue
        .verify_image_admission(&host, &play, &wrong_control)
        .is_err());
    assert!(!queue.status().play_terminal);
    drop(queue);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
}
