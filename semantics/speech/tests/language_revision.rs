#![cfg(feature = "semantic-bindings")]
extern crate alloc;
mod semantic {
    pub use conduit_speech::semantic::*;
}
#[allow(dead_code)]
#[path = "../src/language_revision.rs"]
mod language_revision;
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use language_revision::*;
use semantic::*;
fn envelope(sequence: u64, event: AsrRecognitionEvent) -> AsrRecognitionEnvelope {
    let time = ListeningEventTime::new(0, ListeningClockOrigin::StreamStart).unwrap();
    AsrRecognitionEnvelope::new(
        event,
        ListeningEventId::new(format!("event/{sequence}")).unwrap(),
        ListeningProvenance::new(
            BoundedSequence::new(),
            ListeningProvenanceKind::Direct,
            None,
            None,
            BoundedSequence::new(),
        )
        .unwrap(),
        sequence,
        ListeningStreamId::new("stream".into()).unwrap(),
        ListeningEventTimes::new(time.clone(), time).unwrap(),
    )
    .unwrap()
}
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "asr-revision-bridge".into(),
        "profile/1".into(),
    )
    .unwrap()
}
fn identity(value: &str) -> LanguageTextRevisionId {
    LanguageTextRevisionId::new(value.into()).unwrap()
}
#[test]
fn scalar_replacements_keep_exact_envelope_and_cancellation() {
    let stream = ListeningStreamId::new("stream".into()).unwrap();
    let segment = ListeningSegmentId::new("segment".into()).unwrap();
    let text = LanguageTextId::new("text".into()).unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let basis = AsrLanguageBasis {
        stream: &stream,
        segment: &segment,
        text: &text,
        language: &language,
    };
    let first = envelope(
        3,
        AsrRecognitionEvent::partial_hypothesis(
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            "猫?".into(),
        )
        .unwrap(),
    );
    let PreparedAsrChange::Revision(first_prepared) = prepare_asr_revision(
        &basis,
        &first,
        None,
        identity("r0"),
        provenance(),
        Some(1),
        0,
        8,
    )
    .unwrap() else {
        panic!()
    };
    assert!(core::ptr::eq(first_prepared.envelope(), &first));
    let next = envelope(
        5,
        AsrRecognitionEvent::revised_hypothesis(
            None,
            LanguageTextRange::new(2, 1).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            "!".into(),
        )
        .unwrap(),
    );
    assert!(matches!(
        prepare_asr_revision(
            &basis,
            &next,
            Some(&first_prepared),
            identity("r0"),
            provenance(),
            None,
            0,
            8
        ),
        Err(AsrLanguageRefusal::Revision(
            TextRevisionRefusal::PriorRevision
        ))
    ));
    let PreparedAsrChange::Revision(next_prepared) = prepare_asr_revision(
        &basis,
        &next,
        Some(&first_prepared),
        identity("r1"),
        provenance(),
        None,
        0,
        8,
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(next_prepared.revision().material().text(), "猫!");
    assert_eq!(next_prepared.revision().stable_prefix(), &Some(1));
    assert!(matches!(
        prepare_asr_revision(
            &basis,
            &next,
            Some(&next_prepared),
            identity("r2"),
            provenance(),
            None,
            0,
            8
        ),
        Err(AsrLanguageRefusal::Sequence)
    ));
    assert!(matches!(
        prepare_asr_revision(
            &basis,
            &next,
            Some(&first_prepared),
            identity("bounded"),
            provenance(),
            None,
            0,
            1
        ),
        Err(AsrLanguageRefusal::Revision(
            TextRevisionRefusal::RevisableFrontier
        ))
    ));
    let other_segment = ListeningSegmentId::new("other-segment".into()).unwrap();
    let wrong_segment_basis = AsrLanguageBasis {
        stream: &stream,
        segment: &other_segment,
        text: &text,
        language: &language,
    };
    assert!(matches!(
        prepare_asr_revision(
            &wrong_segment_basis,
            &next,
            Some(&first_prepared),
            identity("wrong-segment"),
            provenance(),
            None,
            0,
            8
        ),
        Err(AsrLanguageRefusal::Segment)
    ));
    let other_text = LanguageTextId::new("other-text".into()).unwrap();
    let wrong_text_basis = AsrLanguageBasis {
        stream: &stream,
        segment: &segment,
        text: &other_text,
        language: &language,
    };
    assert!(matches!(
        prepare_asr_revision(
            &wrong_text_basis,
            &next,
            Some(&first_prepared),
            identity("wrong-text"),
            provenance(),
            None,
            0,
            8
        ),
        Err(AsrLanguageRefusal::Source)
    ));
    let french = ListeningLanguageHypothesis::new(
        None,
        LanguageId::new("language/fr".into()).unwrap(),
        None,
    )
    .unwrap();
    let foreign_commit = envelope(
        6,
        AsrRecognitionEvent::committed_segment(
            None,
            Some(french),
            ListeningTextRole::Recognition,
            segment.clone(),
            None,
            "chat".into(),
            BoundedSequence::new(),
        )
        .unwrap(),
    );
    assert!(matches!(
        prepare_asr_revision(
            &basis,
            &foreign_commit,
            Some(&next_prepared),
            identity("foreign"),
            provenance(),
            None,
            0,
            8
        ),
        Err(AsrLanguageRefusal::Language)
    ));
    let final_event = envelope(
        6,
        AsrRecognitionEvent::committed_segment(
            None,
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            None,
            "猫!".into(),
            BoundedSequence::new(),
        )
        .unwrap(),
    );
    let PreparedAsrChange::Revision(final_prepared) = prepare_asr_revision(
        &basis,
        &final_event,
        Some(&next_prepared),
        identity("final"),
        provenance(),
        None,
        0,
        8,
    )
    .unwrap() else {
        panic!()
    };
    assert!(matches!(
        final_prepared.revision().finality(),
        LanguageTextFinality::Final
    ));
    assert_eq!(final_prepared.revision().stable_prefix(), &Some(1));
    let late = envelope(
        8,
        AsrRecognitionEvent::partial_hypothesis(
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            "猫?".into(),
        )
        .unwrap(),
    );
    let PreparedAsrChange::Revision(correction) = prepare_asr_revision(
        &basis,
        &late,
        Some(&final_prepared),
        identity("late"),
        provenance(),
        None,
        1,
        8,
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(correction.revision().material().text(), "猫?");
    assert!(matches!(
        correction.revision().finality(),
        LanguageTextFinality::Partial
    ));
    assert_eq!(final_prepared.revision().material().text(), "猫!");
    let other_stream = ListeningStreamId::new("other".into()).unwrap();
    let wrong_basis = AsrLanguageBasis {
        stream: &other_stream,
        segment: &segment,
        text: &text,
        language: &language,
    };
    assert!(matches!(
        prepare_asr_revision(
            &wrong_basis,
            &next,
            None,
            identity("r9"),
            provenance(),
            None,
            0,
            8
        ),
        Err(AsrLanguageRefusal::Stream)
    ));
    let cancel = envelope(
        6,
        AsrRecognitionEvent::hypothesis_cancelled(
            "cancelled".into(),
            ListeningTextRole::Recognition,
            segment.clone(),
        )
        .unwrap(),
    );
    let PreparedAsrChange::Cancelled { envelope, previous } = prepare_asr_revision(
        &basis,
        &cancel,
        Some(&next_prepared),
        identity("r2"),
        provenance(),
        None,
        0,
        8,
    )
    .unwrap() else {
        panic!()
    };
    assert!(core::ptr::eq(envelope, &cancel));
    assert_eq!(previous.revision().material().text(), "猫!");
}
#[test]
fn utf8_stability_is_explicit_and_checked() {
    let stream = ListeningStreamId::new("stream".into()).unwrap();
    let segment = ListeningSegmentId::new("segment".into()).unwrap();
    let text = LanguageTextId::new("text".into()).unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let basis = AsrLanguageBasis {
        stream: &stream,
        segment: &segment,
        text: &text,
        language: &language,
    };
    let snapshot = |bytes| {
        AsrTranscriptCandidate::new(
            ListeningSegmentId::new("segment".into()).unwrap(),
            None,
            bytes,
            "猫".into(),
            0,
            None,
            "猫?".into(),
            "?".into(),
        )
        .unwrap()
    };
    assert_eq!(
        stable_scalar_prefix(&basis, "猫?", &snapshot(3)).unwrap(),
        1
    );
    assert!(matches!(
        stable_scalar_prefix(&basis, "猫?", &snapshot(1)),
        Err(AsrLanguageRefusal::StableUtf8Boundary)
    ));
}

#[test]
fn foreign_stability_snapshot_refuses() {
    let stream = ListeningStreamId::new("stream".into()).unwrap();
    let segment = ListeningSegmentId::new("segment".into()).unwrap();
    let text = LanguageTextId::new("text".into()).unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let basis = AsrLanguageBasis {
        stream: &stream,
        segment: &segment,
        text: &text,
        language: &language,
    };
    let snapshot = AsrTranscriptCandidate::new(
        ListeningSegmentId::new("foreign".into()).unwrap(),
        None,
        3,
        "猫".into(),
        0,
        None,
        "猫?".into(),
        "?".into(),
    )
    .unwrap();
    assert!(matches!(
        stable_scalar_prefix(&basis, "猫?", &snapshot),
        Err(AsrLanguageRefusal::Segment)
    ));
    let local = AsrTranscriptCandidate::new(
        segment.clone(),
        None,
        3,
        "猫".into(),
        0,
        None,
        "猫?".into(),
        "?".into(),
    )
    .unwrap();
    assert!(matches!(
        stable_scalar_prefix(&basis, "猫!", &local),
        Err(AsrLanguageRefusal::StableSnapshot)
    ));
}
