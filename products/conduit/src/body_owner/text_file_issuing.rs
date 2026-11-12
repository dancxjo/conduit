//! Local participation in the original sealed Body, using its existing owner
//! journal. No global Ready/Started/Lull or remote recording ACK is invented.
use super::*;
use conduit_body::BodyPlayIdentity;
use conduit_std_host::body_execution::{ImageForeInput, ImageIssuingHostRunReport};
const SCHEMA: &str = "conduit.body/issuing-host-execution@1";

impl Owner {
    /// Run the original issuer's complete local assignment, after all remote
    /// approvals have been frozen for `issued`. Caller-owned adapter/preparation
    /// backing stays under the selected Root's existing caller envelope.
    /// Input memory custody supplies no Source or producer authority. A completed
    /// local file operation must still be matched to the caller's exact content
    /// and reference before any remote evidence can be acknowledged or retired.
    #[allow(dead_code)] // Selected Line ingress calls this after content preparation.
    pub(crate) fn run_selected_issuing_text_file_plan(
        &mut self,
        root: &Path,
        issued: &BodyPlayIdentity,
        write: bool,
        input: &ImageForeInput,
    ) -> Result<ImageIssuingHostRunReport, String> {
        self.ensure_execution_retained(root)?;
        self.persist(root)?;
        state::require_execution_slots(root, 2)?;
        if self.host.is_playing() {
            return Err("issuing file participation requires the original idle Host".into());
        }
        self.host.verify_selected_image_fore_input(input)?;
        let proposed = self
            .session
            .realization()
            .ok_or("original Body proposal required")?;
        if proposed.play.is_some()
            || self
                .host
                .prospective_body_play(&proposed.wake, &proposed.plan)?
                != *issued
        {
            return Err("issuing file request differs from original unstarted Body Play".into());
        }
        if let Some(retained) = &self.selected_file_workset {
            retained.verify(&self.host, &self.session.evidence().body.workset)?;
        }
        let expected_input = if write { "value" } else { "selected" };
        if input.input().front_port_id.as_str() != expected_input
            || input.input().track != ConnectionTrack::Payload
        {
            return Err("issuing file ingress differs from the selected operation".into());
        }
        // A repeated original scope remains refused even after a persistence
        // error. Historical receipts are not instructions for a fresh Boot.
        if self.last_execution.as_ref().is_some_and(|receipt| {
            receipt.get("schema").and_then(serde_json::Value::as_str) == Some(SCHEMA)
                && receipt.get("play") == serde_json::to_value(issued).ok().as_ref()
        }) {
            return Err("original issuing participation already attempted".into());
        }
        let biography = self.session.evidence();
        let ad = self.host.advertisement().clone();
        let admissions = self.admissions.as_ref();
        let last = &mut self.last_execution;
        let pending = &mut self.execution_persistence_pending;
        let control = RunControl::default();
        let host = &mut self.host;
        let report = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("conduit-issuing-file".into())
                .stack_size(4 * 1024 * 1024)
                .spawn_scoped(scope, || {
                    let mut fore = OneOutput::prepare(write);
                    let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                    let mut timer = DeadlineTimer {
                        until: Instant::now() + Duration::from_secs(5),
                        control: control.clone(),
                    };
                    host.run_selected_image_issuing_host_plan_with_fore_to_with_start(
                        BodyRunRequest {
                            wake: &proposed.wake,
                            plan: &proposed.plan,
                            control: &control,
                            keyboard: None,
                        },
                        issued,
                        std::slice::from_ref(input.input()),
                        &mut fore,
                        &mut output,
                        &mut timer,
                        |play, sign| {
                            *last = Some(serde_json::json!({
                                "schema":SCHEMA, "phase":"local-start", "body_id":biography.body_id,
                                "host_id":ad.host_id, "boot_id":ad.boot_id,
                                "plan_id":proposed.plan.plan_id, "wake_id":proposed.wake.wake_id,
                                "play":play, "local_start_sign":sign,
                            }));
                            *pending = true;
                            state::retain_with_source(
                                root,
                                biography,
                                last.as_ref(),
                                admissions,
                                None,
                            )?;
                            *pending = false;
                            Ok(())
                        },
                    )
                })
                .map_err(|e| format!("start issuing file worker: {e}"))?
                .join()
                .map_err(|_| "issuing file worker panicked".to_string())
        })??;
        let local = report.local_execution();
        let execution = local.local_execution();
        // No new BodyLifecycleSession is created. The exact local evidence is
        // archived through the same recoverable owner transaction as other runs.
        self.last_execution = Some(serde_json::json!({
            "schema":SCHEMA, "phase":"local-terminal", "body_id":biography.body_id,
            "host_id":ad.host_id, "boot_id":ad.boot_id,
            "plan_id":proposed.plan.plan_id, "wake_id":proposed.wake.wake_id,
            "play":local.play(), "local_terminal_sign":local.local_terminal_sign(),
            "local_terminal":local.local_terminal(), "failure":local.failure(),
            "cleanup_failure":local.cleanup_failure(),
            "kernel_failure":execution.kernel_failure.map(|failure| serde_json::json!({"code":failure.code.as_str(),"detail":failure.detail})),
            "fore_pending_output":execution.fore_pending_output.as_ref().map(|pending| pending.metadata()),
            "fore_outputs":local.fore_deliveries().iter().map(|value| serde_json::json!({
                "front_port_id":value.front_port_id,"track":value.track,
                "value_kind":value.value_kind,"sequence":value.sequence,"bytes":value.bytes,
            })).collect::<Vec<_>>(),
        }));
        self.execution_persistence_pending = true;
        self.persist(root)?;
        Ok(report)
    }
}

#[cfg(test)]
#[path = "text_file_issuing_tests.rs"]
mod tests;
