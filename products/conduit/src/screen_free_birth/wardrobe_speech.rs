//! Deterministic owner-wardrobe announcements on the selected speaker.
//! These are separately labeled owner-report announcements, not a Face readout
//! or a generated Mask Show. The current Face/Show is only playback basis.

use std::{io::Write, path::Path, sync::mpsc, thread, time::Duration};

use conduit_core::HostAdvertisement;
use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::{
    spoken_face_mask::SpokenBatch, spoken_face_stream_execution::SpokenPlaybackOutcome, RunControl,
    RunControlRequestId,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{command_input::CommandInput, debug_error, selected_playback::SelectedPlayback};

pub(super) struct Announcement<'a, I: CommandInput> {
    pub state_dir: &'a Path,
    pub input: &'a mut I,
    pub selected: &'a SelectedPlayback,
    pub face: &'a Presentation,
    pub show: &'a MaskShow,
    pub host: &'a HostAdvertisement,
}

impl<I: CommandInput> Announcement<'_, I> {
    /// False means the listener interrupted. The caller then discards its
    /// inspected wardrobe so another action cannot rely on unheard choices.
    pub(super) fn speak(
        &mut self,
        report: Option<&Value>,
        lines: &[String],
        output: &mut impl Write,
    ) -> Result<bool, String> {
        if lines.is_empty()
            || lines.len() > 40
            || lines
                .iter()
                .any(|line| line.is_empty() || line.len() > 1024)
        {
            return Err("wardrobe announcement exceeds the bounded spoken contract".into());
        }
        let report_bytes = report
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|error| format!("encode owner wardrobe announcement: {error}"))?;
        let report_sha256 = report_bytes
            .as_ref()
            .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
        for (index, wording) in lines.iter().enumerate() {
            let (current_face, current_host) =
                crate::durable_host_control::local_face_snapshot(self.state_dir)?;
            if current_face != *self.face || current_host != *self.host {
                return Err("owner Face or Host Boot changed before wardrobe speech".into());
            }
            if let Some(source) = report {
                let current = crate::durable_host_control::owner_wardrobe::report(
                    self.state_dir,
                    None,
                    0,
                    None,
                )?;
                if report_basis(&current) != report_basis(source) {
                    return Err("owner wardrobe changed before the next spoken choice".into());
                }
            }
            self.selected.verify_host(self.host)?;
            let wording_sha256 = format!("{:x}", Sha256::digest(wording.as_bytes()));
            let stream_id = crate::durable_host::fresh_identity(
                "wardrobe-announcement",
                &format!(
                    "{}:{index}:{wording_sha256}",
                    report_sha256.as_deref().unwrap_or("local-refusal")
                ),
            );
            let batch =
                SpokenBatch::from_accepted_wording(self.face, self.show, wording, stream_id)
                    .map_err(debug_error)?;
            let control = RunControl::default();
            let stop_id = RunControlRequestId::new("stop/installed-wardrobe-announcement")?;
            let worker_control = control.clone();
            let selected = self.selected.clone();
            let face = self.face.clone();
            let show = self.show.clone();
            let spoken = batch.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            let worker = thread::Builder::new()
                .name("conduit-wardrobe-announcement".into())
                .spawn(move || {
                    let _ = sender.send(selected.play(&face, &show, &spoken, &worker_control));
                })
                .map_err(|error| format!("start wardrobe speech Play: {error}"))?;
            let mut interrupted = false;
            let played = loop {
                match receiver.recv_timeout(Duration::from_millis(25)) {
                    Ok(result) => break result,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        let _ = worker.join();
                        return Err("wardrobe speech worker ended without an outcome".into());
                    }
                    Err(mpsc::RecvTimeoutError::Timeout)
                        if !interrupted && self.input.interrupting_announcement() =>
                    {
                        // Completion can race the listener's next command.
                        // Keep waiting for the actual Play result either way.
                        let _ = control.request_stop(stop_id.clone());
                        interrupted = true;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            };
            if worker.join().is_err() {
                return Err("wardrobe speech worker panicked".into());
            }
            let result = played.map_err(debug_error)?;
            self.selected
                .verify_receipt(self.face, self.show, &batch, &result)?;
            let report_current_after_play = match report {
                Some(source) => {
                    let current = crate::durable_host_control::owner_wardrobe::report(
                        self.state_dir,
                        None,
                        0,
                        None,
                    )?;
                    report_basis(&current) == report_basis(source)
                }
                None => true,
            };
            writeln!(
                output,
                "{}",
                json!({
                    "schema": "conduit.body/owner-wardrobe-announcement@1",
                    "source": if report.is_some() { "owner-report" } else { "local-refusal" },
                    "report_sha256": report_sha256,
                    "owner_plan_id": report.map(|value| &value["owner_plan_id"]),
                    "wardrobe_revision": report.map(|value| &value["wardrobe"]["revision"]),
                    "owner_selected_show_id": report.map(|value| &value["show_id"]),
                    "report_current_after_play": report_current_after_play,
                    "wording_sha256": wording_sha256,
                    "wording": wording,
                    "part": index + 1,
                    "parts": lines.len(),
                    "playback": self.selected.receipt_json(&result),
                })
            )
            .map_err(|error| error.to_string())?;
            if interrupted
                || result.outcome != SpokenPlaybackOutcome::Completed
                || !report_current_after_play
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn report_basis(report: &Value) -> Value {
    json!({
        "body_id": report["body_id"],
        "face_id": report["face_id"],
        "face_revision": report["face_revision"],
        "owner_plan_id": report["owner_plan_id"],
        "wardrobe": report["wardrobe"],
        "admitted_routes": report["admitted_routes"],
        "selected": report["selected"],
        "show_id": report["show_id"],
        "planning": report["reconciliation"]["planning"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_provenance_can_differ_without_changing_the_current_wardrobe() {
        let action_result = json!({
            "body_id":"body/a", "face_id":"face/a", "face_revision":3,
            "owner_plan_id":"plan/a", "wardrobe":{"revision":2},
            "admitted_routes":[], "selected":null, "show_id":null,
            "reconciliation":{"planning":"ReplacementRequired"},
            "transition":{"accepted":"wear"}
        });
        let mut inspection = action_result.clone();
        inspection["transition"] = Value::Null;
        inspection["reconciliation"]["show"] = json!("Retain");
        assert_eq!(report_basis(&action_result), report_basis(&inspection));
        inspection["wardrobe"]["revision"] = json!(3);
        assert_ne!(report_basis(&action_result), report_basis(&inspection));
    }
}
