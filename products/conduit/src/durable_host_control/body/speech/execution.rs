//! Bounded acknowledged-Face reading through the existing selected audio Plan.
use super::{AttachedEquipment, MaskShow, Presentation, RunControl, SpeechFailure, StdHost};
use conduit_std_host::spoken_face_mask::{ReaderCommand, SpokenFaceSession};
use serde_json::{json, Value};
#[path = "batch.rs"]
pub(super) mod batch;

#[derive(Clone, Copy)]
pub(super) enum ReadingScope {
    WholeFace,
    RemainingItems,
}
impl ReadingScope {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::WholeFace => "whole-face",
            Self::RemainingItems => "remaining-items",
        }
    }
    pub(super) fn command(self) -> ReaderCommand {
        match self {
            Self::WholeFace => ReaderCommand::ReadAll,
            Self::RemainingItems => ReaderCommand::ReadCurrentItems,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn play_selected(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    equipment: Option<&AttachedEquipment>,
    control: &RunControl,
    segments_per_batch: usize,
    maximum_batches: usize,
    scope: ReadingScope,
) -> Result<Value, SpeechFailure> {
    let mut receipts = Vec::with_capacity(maximum_batches);
    let result = play_selected_inner(
        host,
        face,
        show,
        equipment,
        control,
        segments_per_batch,
        maximum_batches,
        scope,
        &mut receipts,
    );
    result.map_err(|cause| {
        if receipts.is_empty() {
            cause
        } else {
            SpeechFailure::Partial {
                cause: Box::new(cause),
                completed_batches: receipts,
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn play_selected_inner(
    host: &mut StdHost,
    face: &Presentation,
    show: &MaskShow,
    equipment: Option<&AttachedEquipment>,
    control: &RunControl,
    segments_per_batch: usize,
    maximum_batches: usize,
    scope: ReadingScope,
    receipts: &mut Vec<Value>,
) -> Result<Value, SpeechFailure> {
    if equipment.map_or_else(
        || !host.spoken_artifact_only_route_is_current(),
        |equipment| !equipment.matches(host),
    ) {
        return Err(SpeechFailure::Refused(
            "selected speech equipment changed before Play".into(),
        ));
    }
    let offered = host.advertisement().clone();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(|error| {
        SpeechFailure::Refused(format!("selected spoken Face refused: {error:?}"))
    })?;
    reader
        .command(face, show, scope.command(), 1)
        .map_err(|error| {
            SpeechFailure::Refused(format!(
                "selected spoken {} refused: {error:?}",
                scope.name()
            ))
        })?;
    let mut terminal_turn = None;
    for _ in 0..maximum_batches {
        if control.stop_requested() {
            return Err(SpeechFailure::Cancelled(
                "selected speech stop requested".into(),
            ));
        }
        let Some(batch) = reader
            .next_batch_with_limits(segments_per_batch, 64)
            .map_err(|error| {
                SpeechFailure::Refused(format!("selected speech batch refused: {error:?}"))
            })?
        else {
            break;
        };
        let (receipt, delivery) = batch::play_batch(host, face, show, &batch, equipment, control)?;
        receipts.push(receipt);
        let terminal = reader.acknowledge_batch(delivery).map_err(|error| {
            SpeechFailure::Failed(format!("selected speaker receipt refused: {error:?}"))
        })?;
        if let Some(terminal) = terminal {
            terminal_turn = Some(terminal);
            break;
        }
    }
    let terminal = terminal_turn.ok_or_else(|| {
        SpeechFailure::Refused(format!(
            "selected Face reading exceeded {maximum_batches} admitted speech batches"
        ))
    })?;
    if receipts.is_empty()
        || terminal.outcome != conduit_std_host::spoken_face_mask::SpokenTurnOutcome::Completed
    {
        return Err(SpeechFailure::Failed(
            "selected speech ended without bounded complete readout".into(),
        ));
    }
    Ok(json!({"schema":"conduit.body/selected-speech-terminal@1",
        "outcome":"completed", "reader_scope":scope.name(), "face_id":face.identity.as_str(),
        "face_revision":face.revision,
        "face_revision_decimal":face.revision.to_string(),
        "source_show_id":show.show_id.as_str(),
        "host_id":offered.host_id.as_str(), "boot_id":offered.boot_id.as_str(),
        "offer_generation":offered.offer_generation.0,
        "provider_sha256":equipment.map(|equipment| equipment.provider_sha256.as_str()),
        "output_mode":if equipment.is_some() { "speaker" } else { "wav-artifact" },
        "selected_resource_pool_id":equipment.map(|equipment| equipment.playback.pool_id().as_str().to_owned()),
        "authority_grant_id":equipment.map(|equipment| equipment.authorization.grant_id()),
        "completed_segments":terminal.completed_segments,
        "produced_pcm_bytes":terminal.produced_pcm_bytes,
        "correlation_sha256":terminal.correlation_sha256,
        "batches":std::mem::take(receipts)}))
}
