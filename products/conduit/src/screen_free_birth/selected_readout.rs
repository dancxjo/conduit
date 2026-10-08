//! Selected spoken reading of one exact installed Face and acknowledged Show.

use std::{io::Write, path::Path, sync::mpsc, thread, time::Duration};

use conduit_core::HostAdvertisement;
use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::spoken_face_mask::{
    ReaderCommand, SpokenBatchDelivery, SpokenFaceSession, SpokenTurnOutcome,
};
use conduit_std_host::{RunControl, RunControlRequestId};

use super::{command_input::CommandInput, debug_error, selected_playback::SelectedPlayback};
use crate::durable_host_control;

#[derive(Clone, Copy)]
pub(super) enum OutputPhase {
    Birth,
    Body,
    Wardrobe,
}

fn face_is_current(
    state_dir: &Path,
    face: &Presentation,
    advertisement: &HostAdvertisement,
    phase: OutputPhase,
) -> Result<bool, String> {
    let (current, host) = match phase {
        OutputPhase::Birth => durable_host_control::birth_face(state_dir)?,
        OutputPhase::Body => durable_host_control::local_face_snapshot(state_dir)?,
        OutputPhase::Wardrobe => {
            let (base, host) = durable_host_control::local_face_snapshot(state_dir)?;
            let (_, reading) = durable_host_control::local_wardrobe(
                state_dir,
                base.basis
                    .body_id
                    .clone()
                    .ok_or("owner wardrobe lost its Body")?,
                base.identity.as_str().into(),
                base.revision,
                host.clone(),
                None,
                0,
                None,
            )?;
            if host != *advertisement {
                return Err("installed Host Boot changed during selected speech Play".into());
            }
            let Ok(reading) = reading else {
                return Ok(false);
            };
            (reading, host)
        }
    };
    if host != *advertisement {
        return Err("installed Host Boot changed during selected speech Play".into());
    }
    Ok(current == *face)
}

fn stop_stale_reading(
    reader: &mut SpokenFaceSession,
    face: &Presentation,
    show: &MaskShow,
    sequence: &mut u64,
    output: &mut impl Write,
) -> Result<bool, String> {
    *sequence = sequence.checked_add(1).ok_or("input sequence exhausted")?;
    let stopped = reader
        .command(face, show, ReaderCommand::Stop, *sequence)
        .map_err(debug_error)?;
    if let Some(terminal) = stopped.interrupted {
        write_turn(&terminal, output)?;
    }
    writeln!(output, "Owner Face changed during speech. Stopped the stale reading; read all again for the current Face.")
        .map_err(|error| error.to_string())?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
/// Return true when the current spoken turn was interrupted, so an opening
/// guide cannot start a new ReadAll after the person stopped it.
pub(super) fn emit_readout(
    state_dir: &Path,
    input: &mut impl CommandInput,
    playback: Option<&SelectedPlayback>,
    reader: &mut SpokenFaceSession,
    face: &Presentation,
    show: &MaskShow,
    advertisement: &HostAdvertisement,
    sequence: &mut u64,
    phase: OutputPhase,
    output: &mut impl Write,
) -> Result<bool, String> {
    let Some(selected) = playback else {
        return super::emit_readout(reader, face, show, None, output).map(|_| false);
    };
    selected.verify_host(advertisement)?;
    loop {
        if !face_is_current(state_dir, face, advertisement, phase)? {
            if matches!(phase, OutputPhase::Body | OutputPhase::Wardrobe) {
                return stop_stale_reading(reader, face, show, sequence, output);
            }
            return Err("installed Birth Face changed before the next speech Play".into());
        }
        // Keep a concise Face turn in one selected speaker Play and its
        // committed-PCM WAV where it fits. Stop still cancels that Play.
        let Some(batch) = reader.next_batch_with_limits(4, 64).map_err(debug_error)? else {
            return Ok(false);
        };
        let control = RunControl::default();
        let stop_sequence = sequence.checked_add(1).ok_or("input sequence exhausted")?;
        let stop_id = RunControlRequestId::new("stop/installed-spoken-face")?;
        let running = control.clone();
        let selected_for_play = selected.clone();
        let source_face = face.clone();
        let source_show = show.clone();
        let source_batch = batch.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("conduit-spoken-face-play".into())
            .spawn(move || {
                let result =
                    selected_for_play.play(&source_face, &source_show, &source_batch, &running);
                let _ = sender.send(result);
            })
            .map_err(|error| format!("start selected speech Play: {error}"))?;
        let mut interrupted = false;
        let mut interruption_error = None;
        let played = loop {
            match receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(result) => break result,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = worker.join();
                    let reason = "selected speech Play stopped without an outcome";
                    let terminal = reader
                        .acknowledge_batch(SpokenBatchDelivery::Failed(reason.into()))
                        .map_err(debug_error)?;
                    if let Some(terminal) = terminal {
                        write_turn(&terminal, output)?;
                    }
                    return Err(reason.into());
                }
                Err(mpsc::RecvTimeoutError::Timeout)
                    if !interrupted
                        && input.interrupting_command(reader, face, show, stop_sequence) =>
                {
                    *sequence = stop_sequence;
                    match reader.command(face, show, ReaderCommand::Stop, *sequence) {
                        Ok(stopped)
                            if stopped.cancel_stream_identity.as_deref()
                                == Some(batch.stream_identity.as_str()) => {}
                        Ok(_) => {
                            interruption_error = Some("Stop named a different spoken stream".into())
                        }
                        Err(refusal) => interruption_error = Some(debug_error(refusal)),
                    }
                    if let Err(refusal) = control.request_stop(stop_id.clone()) {
                        interruption_error.get_or_insert_with(|| debug_error(refusal));
                    }
                    interrupted = true;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        };
        if worker.join().is_err() {
            let reason = "selected speech worker panicked";
            let terminal = reader
                .acknowledge_batch(SpokenBatchDelivery::Failed(reason.into()))
                .map_err(debug_error)?;
            if let Some(terminal) = terminal {
                write_turn(&terminal, output)?;
            }
            return Err(reason.into());
        }
        if let Some(reason) = interruption_error {
            let terminal = reader
                .acknowledge_batch(SpokenBatchDelivery::Failed(reason.clone()))
                .map_err(debug_error)?;
            if let Some(terminal) = terminal {
                write_turn(&terminal, output)?;
            }
            return Err(reason);
        }
        let result = match played {
            Ok(result) => result,
            Err(refusal) => {
                let reason = format!("selected speech Play refused: {refusal:?}");
                let terminal = reader
                    .acknowledge_batch(SpokenBatchDelivery::Failed(reason.clone()))
                    .map_err(debug_error)?;
                if let Some(terminal) = terminal {
                    write_turn(&terminal, output)?;
                }
                return Err(reason);
            }
        };
        let spoken_segments = match selected.verify_receipt(face, show, &batch, &result) {
            Ok(segments) => segments,
            Err(refusal) => {
                let terminal = reader
                    .acknowledge_batch(SpokenBatchDelivery::Failed(refusal.clone()))
                    .map_err(debug_error)?;
                if let Some(terminal) = terminal {
                    write_turn(&terminal, output)?;
                }
                return Err(refusal);
            }
        };
        let terminal = reader
            .acknowledge_batch(result.delivery())
            .map_err(debug_error)?;
        writeln!(
            output,
            "{}",
            selected.receipt_json(&result, &spoken_segments)
        )
        .map_err(|error| error.to_string())?;
        if let Some(terminal) = &terminal {
            write_turn(terminal, output)?;
        }
        if let Some(terminal) = terminal {
            return Ok(turn_interrupted(&terminal.outcome));
        }
        if !face_is_current(state_dir, face, advertisement, phase)? {
            if matches!(phase, OutputPhase::Body | OutputPhase::Wardrobe) {
                return stop_stale_reading(reader, face, show, sequence, output);
            }
            return Err("installed Birth Face changed during selected speech Play".into());
        }
    }
}

pub(super) fn turn_interrupted(outcome: &SpokenTurnOutcome) -> bool {
    matches!(outcome, SpokenTurnOutcome::Cancelled)
}

fn write_turn(
    terminal: &conduit_std_host::spoken_face_mask::SpokenTurnReceipt,
    output: &mut impl Write,
) -> Result<(), String> {
    writeln!(
        output,
        "{}",
        serde_json::json!({
            "schema": "conduit.body/spoken-face-turn@1",
            "face_id": terminal.face_id,
            "face_revision": terminal.face_revision,
            "face_revision_decimal": terminal.face_revision.to_string(),
            "source_show_id": terminal.show_id,
            "outcome": format!("{:?}", terminal.outcome),
            "completed_segments": terminal.completed_segments,
            "committed_pcm_bytes": terminal.produced_pcm_bytes,
            "provider_sha256": terminal.provider_sha256,
            "correlation_sha256": terminal.correlation_sha256,
        })
    )
    .map_err(|error| error.to_string())
}
