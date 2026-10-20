//! Ordinary numerical Fore ownership over one sealed Body Play.
//! The queue is a producer boundary; scheduling and numeric work stay in the kernel.
use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunReport, BodyRunRequest},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, RunControl,
    RunControlRequestId, StdHost, ThreadTimer,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::*;
use std::{sync::mpsc, thread::JoinHandle, time::Duration};

pub struct NumericalFore {
    queue: BodyLiveForeQueue,
    control: RunControl,
    outputs: mpsc::Receiver<Vec<u8>>,
    worker: Option<JoinHandle<Result<BodyRunReport, String>>>,
    play: BodyPlayIdentity,
    body_plan: BodyPlan,
    verifier: PreparedNearQuantityVerifier,
    limit: RationalQuantity,
}
pub struct NumericalValue {
    pub value: VerifiedNearQuantity,
    pub canonical: Vec<u8>,
    pub play: BodyPlayIdentity,
}
struct Capture(mpsc::SyncSender<Vec<u8>>);
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        self.0
            .send(output.bytes.to_vec())
            .map_err(|_| "numerical Fore consumer stopped".into())
    }
}
fn error(why: impl core::fmt::Debug) -> String {
    format!("{why:?}")
}
impl NumericalFore {
    /// Consume the real installed Host and its already checked/planned Source.
    pub fn from_plan(mut host: StdHost, plan: Plan) -> Result<Self, String> {
        let Some(PlannedActivationEntry::Unary(each)) = plan.activations.first() else {
            return Err("numerical Fore has no Each activation".into());
        };
        crate::flow_activation::validate_near_each(&plan, each)?;
        let child = &each.selected_plan.fragments[0].placements[0];
        let [ConfigurationEntry {
            value: ConfigurationValue::Quantity(limit),
            ..
        }] = child.configuration.as_slice()
        else {
            return Err("numerical Fore has no sealed physical limit".into());
        };
        let limit =
            RationalQuantity::from_quantity(limit.value(), &mut ExactNumericWork::new(200_000_000))
                .map_err(error)?;
        let verifier =
            PreparedNearQuantityVerifier::new(&conduit_semantic_catalog::near_quantity_type())
                .map_err(error)?;
        let body = Body::born(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
            1,
            "sign/numerical-fore-born".into(),
        )
        .map_err(error)?;
        let wake = body
            .wake(1, "sign/numerical-fore-wake".into())
            .map_err(error)?
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
        .map_err(error)?;
        let control = RunControl::default();
        let queue =
            BodyLiveForeQueue::for_near_each_plan(&body_plan.plots[0].plan, control.clone(), 1)?;
        let retained = body_plan.clone();
        let producer = queue.clone();
        let run_control = control.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (output_tx, outputs) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("numerical-body-play".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || {
                host.run_body_plan_with_live_fore_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body_plan,
                        control: &run_control,
                        keyboard: None,
                    },
                    &producer,
                    &mut Capture(output_tx),
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |play, _| started_tx.send(play.clone()).map_err(error),
                )
            })
            .map_err(error)?;
        let play = match started_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(play) => play,
            Err(why) => {
                let _ = control.request_stop(RunControlRequestId::new("numerical-start-refused")?);
                let result = worker.join().map_err(error)?;
                return Err(format!("numerical Body did not start: {why}; {result:?}"));
            }
        };
        Ok(Self {
            queue,
            control,
            outputs,
            worker: Some(worker),
            play,
            body_plan: retained,
            verifier,
            limit,
        })
    }
    pub fn body_plan(&self) -> &BodyPlan {
        &self.body_plan
    }
    pub fn play(&self) -> &BodyPlayIdentity {
        &self.play
    }
    pub fn execute(&mut self, value: RationalQuantity) -> Result<NumericalValue, String> {
        let mut bytes = [0_u8; QUANTITY_VALUE_MAX_ENCODED_BYTES];
        let length = encode_rational_quantity_value(value, &mut bytes).map_err(error)?;
        self.execute_encoded(&bytes[..length])
    }
    pub fn execute_encoded(&mut self, bytes: &[u8]) -> Result<NumericalValue, String> {
        if !matches!(
            self.queue.submit(bytes)?,
            BodyLiveForeAdmission::Accepted { .. }
        ) {
            return Err("numerical Fore queue is full".into());
        }
        let canonical = match self.outputs.recv_timeout(Duration::from_secs(30)) {
            Ok(output) => output,
            Err(why) => {
                let _ = self
                    .control
                    .request_stop(RunControlRequestId::new("numerical-output-unavailable")?);
                // The caller can still finish/join and inspect the exact terminal
                // report and complete child evidence after an execution refusal.
                return Err(format!("numerical Body produced no accepted output: {why}"));
            }
        };
        let source = semantic_digest(QUANTITY_INFO_ID, bytes);
        let value = self
            .verifier
            .verify(
                &canonical,
                QUANTITY_INFO_ID,
                self.limit,
                Some(source),
                &mut ExactNumericWork::new(200_000_000),
            )
            .map_err(error)?;
        Ok(NumericalValue {
            value,
            canonical,
            play: self.play.clone(),
        })
    }
    pub fn finish(mut self) -> Result<BodyRunReport, String> {
        if !self.queue.status().play_terminal && !self.control.stop_requested() {
            self.queue.close()?;
        }
        self.worker
            .take()
            .ok_or("numerical Body already joined")?
            .join()
            .map_err(error)?
    }
    pub fn stop(mut self) -> Result<BodyRunReport, String> {
        self.control
            .request_stop(RunControlRequestId::new("numerical-explicit-stop")?)
            .map_err(error)?;
        self.worker
            .take()
            .ok_or("numerical Body already joined")?
            .join()
            .map_err(error)?
    }
}
impl Drop for NumericalFore {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.control.request_stop(
                RunControlRequestId::new("numerical-fore-dropped").expect("static Stop identity"),
            );
            let _ = worker.join();
        }
    }
}
