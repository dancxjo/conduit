//! Ordinary authored scan, installed std Host, sealed BodyPlan and live Play.
use conduit_body::{Body, BodyId, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{CanonicalBackCatalog, ProfileCatalog, StartupCatalog};
use conduit_std_host::{
    body_execution::{BodyForeOutputAdapter, BodyRunReport, BodyRunRequest},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, RunControl,
    RunControlRequestId, StdHost, StdHostConfig, TimerAdapter,
};
use conduit_thermostat_plot::*;
use std::{collections::BTreeMap, sync::mpsc, thread::JoinHandle, time::Duration};

pub struct Execution {
    queue: BodyLiveForeQueue,
    control: RunControl,
    outputs: mpsc::Receiver<Result<ThermostatState, String>>,
    worker: Option<JoinHandle<Result<BodyRunReport, String>>>,
    basis: conduit_presentation::PresentationContributionBasis,
    body_id: BodyId,
    body_plan: BodyPlan,
}
pub struct ResultState {
    pub state: ThermostatState,
    pub basis: conduit_presentation::PresentationContributionBasis,
    pub body_id: BodyId,
}
fn error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}
struct Capture(mpsc::SyncSender<Result<ThermostatState, String>>);
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        self.0
            .send(ThermostatState::decode(&value.bytes).map_err(error))
            .map_err(error)
    }
}
fn authored_plan(host: &StdHost) -> Result<Plan, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_catalogs(&mut startup, &mut profile)?;
    let document = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../main.conduit")),
        &startup,
    )
    .map_err(error)?;
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&document, "thermostat/main", &profile)
            .map_err(error)?;
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authoring.expanded, &hosts).map_err(error)?;
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: COMMAND_BYTES as u32,
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
                byte_capacity: STATE_BYTES as u32,
            },
        ),
    ]);
    conduit_planner::plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty_bases,
            line_candidates: &empty_lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &bounds,
    )
    .map_err(error)
}
impl Execution {
    pub fn new() -> Result<Self, String> {
        let boot = format!(
            "thermostat/boot/{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(error)?
                .as_nanos()
        );
        let host = StdHost::new_for_thermostat_scan(
            StdHostConfig {
                host_id: HostId::from("std-thermostat"),
                boot_id: boot.into(),
                offer_generation: OfferGeneration(1),
            },
            &ThermostatState::default(),
            256,
        )?;
        let plan = authored_plan(&host)?;
        Self::from_plan(host, plan)
    }
    /// Play the caller's checked and planned source on its scoped installed Host.
    pub fn from_plan(mut host: StdHost, plan: Plan) -> Result<Self, String> {
        let checked_plot_id = plan.checked_plot_id.clone();
        let body = Body::born(
            plan.source_document_id.clone(),
            checked_plot_id.clone(),
            1,
            SignId::from("sign/thermostat-born"),
        )
        .map_err(error)?;
        let wake = body
            .wake(1, SignId::from("sign/thermostat-wake"))
            .map_err(error)?
            .1;
        let body_plan = BodyPlan::seal(
            &wake,
            vec![BodyPlotPlan {
                plot: ResidentPlot::new(plan.source_document_id.clone(), checked_plot_id.clone()),
                plan,
            }],
        )
        .map_err(error)?;
        let control = RunControl::default();
        let queue =
            BodyLiveForeQueue::for_thermostat_plan(&body_plan.plots[0].plan, control.clone(), 1)?;
        let retained_body_plan = body_plan.clone();
        let worker_queue = queue.clone();
        let worker_control = control.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (output_tx, outputs) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("thermostat-body-play".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || {
                host.run_body_plan_with_live_fore_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &worker_control,
                        keyboard: None,
                    },
                    &worker_queue,
                    &mut Capture(output_tx),
                    &mut Vec::new(),
                    &mut NoTimer,
                    |play, _| started_tx.send(play.clone()).map_err(error),
                )
            })
            .map_err(error)?;
        let play = match started_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(play) => play,
            Err(e) => {
                let _ = control.request_stop(
                    RunControlRequestId::new("thermostat-start-refused")
                        .expect("static Stop identity"),
                );
                let report = worker.join().map_err(error)?;
                return Err(format!("Body Play did not start: {e}; {report:?}"));
            }
        };
        Ok(Self {
            queue,
            control,
            outputs,
            worker: Some(worker),
            body_id: play.body_id,
            body_plan: retained_body_plan,
            basis: conduit_presentation::PresentationContributionBasis {
                checked_plot_id,
                plan_id: play.plan_id,
                active_play_id: play.active_play_id,
                required_interaction_context: None,
            },
        })
    }
    pub fn accepts_actions(&self) -> bool {
        let status = self.queue.status();
        let Some(PlannedActivationEntry::Scan(scan)) =
            self.body_plan.plots[0].plan.activations.first()
        else {
            return false;
        };
        !status.play_terminal
            && !status.close_requested
            && !self.control.stop_requested()
            && status.queue_accepted < scan.limits.maximum_items
    }
    pub fn body_plan(&self) -> &BodyPlan {
        &self.body_plan
    }
    pub fn stop(mut self) -> Result<BodyRunReport, String> {
        self.control
            .request_stop(RunControlRequestId::new("thermostat-explicit-stop")?)
            .map_err(error)?;
        self.worker
            .take()
            .ok_or("Body Play already stopped")?
            .join()
            .map_err(error)?
    }
    pub fn execute(&mut self, command: Command) -> Result<ResultState, String> {
        if !matches!(
            self.queue.submit(&command.encode())?,
            BodyLiveForeAdmission::Accepted { .. }
        ) {
            return Err("thermostat command queue is full".into());
        }
        let state = match self.outputs.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(state)) => state,
            outcome => {
                let _ = self
                    .control
                    .request_stop(RunControlRequestId::new("thermostat-output-refused")?);
                if let Some(worker) = self.worker.take() {
                    let _ = worker.join();
                }
                return Err(format!(
                    "Thermostat output refused; Play stopped: {outcome:?}"
                ));
            }
        };
        Ok(ResultState {
            state,
            basis: self.basis.clone(),
            body_id: self.body_id.clone(),
        })
    }
    pub fn finish(mut self) -> Result<BodyRunReport, String> {
        self.queue.close()?;
        self.worker
            .take()
            .ok_or("Body Play already stopped")?
            .join()
            .map_err(error)?
    }
}
impl Drop for Execution {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.control.request_stop(
                RunControlRequestId::new("thermostat-encounter-stop")
                    .expect("static Stop identity"),
            );
            let _ = worker.join();
        }
    }
}
