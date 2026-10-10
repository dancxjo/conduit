//! Return an acknowledged Mask interaction to the existing Body scan Fore.
use super::{body::HostSource, body_run::OwnedRunWorker, DurableHostRuntime};
use conduit_presentation::{FaceInteraction, MaskShow};
use conduit_std_host::BodyLiveForeAdmission;
use std::time::{Duration, Instant};
impl DurableHostRuntime {
    pub(super) fn submit_owned_thermostat_action(
        &mut self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let HostSource::Body {
            owner,
            root,
            running,
        } = &mut self.host
        else {
            return Err("Thermostat does not own a Body".into());
        };
        let Some(OwnedRunWorker::Thermostat(worker)) = running else {
            return Err("Thermostat has no current scan Play".into());
        };
        if worker.submit_interaction(owner, show, interaction)? == BodyLiveForeAdmission::Full {
            return Err("Thermostat command Fore is full".into());
        }
        let deadline = Instant::now()
            + super::body::CONTROL_DEADLINE.saturating_sub(Duration::from_millis(250));
        loop {
            if worker.progress(owner, root)? {
                let observed = !worker.output_pending();
                *running = None;
                if observed {
                    let receipt = owner
                        .thermostat_terminal_receipt()
                        .ok_or("Thermostat terminal receipt is unavailable")?;
                    return Ok(serde_json::json!({
                        "schema":"conduit.thermostat/observed-terminal-action@1",
                        "body_id":show.show.body_id, "interaction_id":interaction.identity,
                        "prior_show_id":show.show_id, "terminal":true,
                        "terminal_disposition":receipt["terminal"],
                        "terminal_sign":receipt["terminal_sign"],
                        "failure":receipt["failure"],
                        "cleanup_failure":receipt["cleanup_failure"],
                        "kernel_failure":receipt["kernel_failure"],
                    }));
                }
                return Err("Thermostat Play ended before state output".into());
            }
            if !worker.output_pending() {
                let face = owner.local_face_snapshot()?;
                return Ok(serde_json::json!({
                    "schema":"conduit.thermostat/observed-action@1",
                    "body_id":face.basis.body_id, "interaction_id":interaction.identity,
                    "prior_show_id":show.show_id, "face_id":face.identity,
                    "face_revision":face.revision, "active_play_id":owner.current_play_id(),
                }));
            }
            if Instant::now() >= deadline {
                // A timeout cannot admit another command against stale state.
                worker.request_lull()?;
                return Err(super::CONTROL_OUTCOME_UNKNOWN.into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
