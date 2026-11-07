//! Reference-only execution for isolated lifecycle fixtures, never installed Run.
use super::*;
use conduit_std_host::body_execution::BodyRunRequest;
impl Owner {
    pub(super) fn execute_reference_fixture(&mut self, maximum_millis: u64) -> Result<(), String> {
        if !(1..=60_000).contains(&maximum_millis) {
            return Err("run duration must be 1..60000 milliseconds".into());
        }
        let proposed = self
            .session
            .realization()
            .ok_or("plan required before run")?
            .clone();
        if proposed
            .plan
            .plots
            .iter()
            .any(|plot| !plot.plan.activations.is_empty())
        {
            return Err("installed Body Play has no activation ingress or egress".into());
        }
        if proposed
            .plan
            .plots
            .iter()
            .flat_map(|plot| &plot.plan.fragments)
            .any(|fragment| fragment.host_id != self.host.advertisement().host_id)
        {
            return Err("remote-fragment-preparation-unavailable".into());
        }
        let control = RunControl::default();
        let mut timer = DeadlineTimer {
            until: Instant::now() + Duration::from_millis(maximum_millis),
            control: control.clone(),
        };
        let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
        let result = std::thread::scope(|scope| {
            let (finished, receiver) = std::sync::mpsc::sync_channel::<()>(1);
            let stop = control.clone();
            scope.spawn(move || {
                if receiver
                    .recv_timeout(Duration::from_millis(maximum_millis))
                    .is_err()
                {
                    let _ = stop.request_stop(
                        RunControlRequestId::new("owner/run-deadline").expect("bounded request"),
                    );
                }
            });
            let result = self.host.run_body_plan_to(
                BodyRunRequest {
                    wake: &proposed.wake,
                    plan: &proposed.plan,
                    control: &control,
                    keyboard: None,
                },
                &mut output,
                &mut timer,
            );
            let _ = finished.send(());
            result
        });
        match result {
            Ok(report) => {
                let host = self.host.advertisement();
                self.session
                    .started(
                        &host.host_id,
                        &host.boot_id,
                        report.play.clone(),
                        report.wake_at_start.clone(),
                    )
                    .map_err(debug)?;
                self.last_execution = Some(
                    serde_json::json!({"host_id":host.host_id, "boot_id":host.boot_id, "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id, "wake_id":proposed.wake.wake_id, "play":report.play, "wake_at_start":report.wake_at_start, "terminal":report.terminal, "failure":report.failure, "cleanup_failure":report.cleanup_failure, "terminal_sign":report.terminal_sign, "output_utf8":String::from_utf8_lossy(&output.0)}),
                );
                self.execution_persistence_pending = true;
                self.session
                    .lull(&host.host_id, &host.boot_id, Some(&report.play))
                    .map_err(debug)?;
                Ok(())
            }
            Err(error) => {
                self.last_execution = Some(
                    serde_json::json!({"host_id":self.host.advertisement().host_id, "boot_id":self.host.advertisement().boot_id, "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id, "wake_id":proposed.wake.wake_id, "play":null, "pre_play_refusal":error, "refusal_scope":"untyped-host-admission"}),
                );
                self.execution_persistence_pending = true;
                // The Host API returns an untyped error. Keep the failed proposal
                // and exact durable refusal; do not invent resource rejection facts.
                Err(error)
            }
        }
    }
}
