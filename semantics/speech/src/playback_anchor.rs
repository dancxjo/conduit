//! Scoped revision anchors backed by complete individually admitted tapes.
use crate::{playback_basis::PreparedSpeechPlaybackTape, semantic::*};
use conduit_plot::rust_binding::NativeBindingRefusal;
/// A scoped header/material anchor; exact complete receipts stay on the change.
pub fn prepare_playback_anchor(
    tape: &PreparedSpeechPlaybackTape<'_>,
) -> Result<SpeechPlaybackRevisionAnchor, NativeBindingRefusal> {
    tape.anchor(|| admit_anchor(tape))
}
fn admit_anchor(
    tape: &PreparedSpeechPlaybackTape<'_>,
) -> Result<SpeechPlaybackRevisionAnchor, NativeBindingRefusal> {
    let basis = tape.basis();
    let first = basis.links().linguistic_bases().iter().next().ok_or(
        NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType),
    )?;
    let anchor = SpeechPlaybackRevisionAnchor::new(
        *basis.clock_id(),
        basis.intent().inventory_id().clone(),
        basis.intent().language().clone(),
        first.material().clone(),
        basis.intent().revision_id().clone(),
        basis.intent().utterance_id().clone(),
        basis.voice_profile().identity().clone(),
    )?;
    SpeechPlaybackAnchorMatch::new(anchor.clone(), basis.clone())?;
    for row in basis.links().linguistic_bases().iter() {
        SpeechPlaybackAnchorMaterialMatch::new(anchor.clone(), row.material().clone())?;
    }
    Ok(anchor)
}

pub(crate) fn prepare_playback_lineage(
    previous: &SpeechPlaybackRevisionAnchor,
    next: &SpeechPlaybackRevisionAnchor,
) -> Result<conduit_language::LanguageTextRevisionLineage, NativeBindingRefusal> {
    let lineage =
        conduit_language::prepare_text_revision_lineage(previous.material(), next.material())?;
    if previous.material() != next.material() {
        // Check the actual retained old text, including its known stable prefix.
        // Played frame commitment is independent of this scalar text frontier.
        conduit_language::validate_text_revision(
            Some(previous.material()),
            next.material(),
            0,
            4096,
        )
        .map_err(|_| {
            NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)
        })?;
    }
    Ok(lineage)
}
