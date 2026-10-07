use conduit_speech::{playback_basis::*, playback_revision::*, semantic::*};
pub fn native_anchor_refusals(tape: &PreparedSpeechPlaybackTape<'_>) {
    use conduit_plot::rust_binding::BoundedSequence;
    let anchor = prepare_playback_anchor(tape).unwrap();
    let old = anchor.material();
    let material = old.material();
    let altered = conduit_language::LanguageText::new(
        material.identity().clone(),
        material.language().clone(),
        material.revision().clone(),
        "same IDs, different full material".into(),
    )
    .unwrap();
    let changed = conduit_language::LanguageTextRevision::new(
        *old.finality(),
        altered,
        old.prior().clone(),
        old.provenance().clone(),
        *old.sequence(),
        *old.stable_prefix(),
    )
    .unwrap();
    let foreign = SpeechPlaybackRevisionAnchor::new(
        *anchor.clock_id(),
        anchor.inventory_id().clone(),
        anchor.language().clone(),
        changed.clone(),
        anchor.revision_id().clone(),
        anchor.utterance_id().clone(),
        anchor.voice_profile().clone(),
    )
    .unwrap();
    assert!(SpeechPlaybackAnchorMatch::new(foreign.clone(), tape.basis().clone()).is_err());
    assert!(SpeechPlaybackAnchorMaterialMatch::new(anchor.clone(), changed).is_err());
    let empty = SpeechPlaybackLinks::new(
        BoundedSequence::try_from_iter([]).unwrap(),
        BoundedSequence::try_from_iter([]).unwrap(),
    )
    .unwrap();
    let empty_basis = SpeechPlaybackBasis::new(
        *tape.basis().clock_id(),
        tape.basis().intent().clone(),
        empty,
        tape.basis().voice_profile().clone(),
    )
    .unwrap();
    assert!(SpeechPlaybackAnchorMatch::new(anchor, empty_basis).is_err());
}

pub fn lineage_refusals(
    tape: &PreparedSpeechPlaybackTape<'_>,
    next_tape: &PreparedSpeechPlaybackTape<'_>,
) {
    let old = prepare_playback_anchor(tape).unwrap();
    let next = prepare_playback_anchor(next_tape).unwrap();
    let lineage =
        conduit_language::prepare_text_revision_lineage(old.material(), next.material()).unwrap();
    assert!(SpeechPlaybackCorrection::new(lineage.clone(), old.clone(), next.clone()).is_ok());
    assert!(SpeechPlaybackInterpretationChange::new(
        Some(lineage.clone()),
        next.clone(),
        Some(old.clone())
    )
    .is_ok());
    assert!(
        SpeechPlaybackInterpretationChange::new(None, next.clone(), Some(old.clone())).is_err()
    );
    assert!(
        SpeechPlaybackInterpretationChange::new(Some(lineage.clone()), next.clone(), None).is_err()
    );
    assert!(SpeechPlaybackCorrection::new(lineage, old.clone(), old.clone()).is_err());
    let unchanged =
        conduit_language::prepare_text_revision_lineage(old.material(), old.material()).unwrap();
    assert!(SpeechPlaybackCorrection::new(unchanged.clone(), old.clone(), old.clone()).is_ok());
    assert!(SpeechPlaybackCorrection::new(unchanged, old.clone(), next.clone()).is_err());
    let body = next.material();
    let foreign_prior = conduit_language::LanguageTextPriorRevision::new(
        conduit_language::LanguageTextRevisionId::new("foreign/prior".into()).unwrap(),
        *old.material().sequence(),
    )
    .unwrap();
    let forged = conduit_language::LanguageTextRevision::new(
        *body.finality(),
        body.material().clone(),
        Some(foreign_prior),
        body.provenance().clone(),
        *body.sequence(),
        *body.stable_prefix(),
    )
    .unwrap();
    assert!(conduit_language::prepare_text_revision_lineage(old.material(), &forged).is_err());
}
