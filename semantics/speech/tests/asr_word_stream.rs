#![cfg(feature = "semantic-bindings")]
//! Checked recognition events supplied by this test, not provider accuracy.
use conduit_language::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{language_revision::*, semantic::*};

#[test]
#[ignore = "exports exact ASR-origin revisions for a separate actual parser run"]
fn export_checked_asr_word_stream() {
    let stream = ListeningStreamId::new("asr/word-stream".into()).unwrap();
    let segment = ListeningSegmentId::new("asr/word-stream/segment".into()).unwrap();
    let text = LanguageTextId::new("stream/v2/independent-hello-travis".into()).unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let basis = AsrLanguageBasis {
        stream: &stream,
        segment: &segment,
        text: &text,
        language: &language,
    };
    let events = [
        AsrRecognitionEvent::partial_hypothesis(
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            "Hello ".into(),
        )
        .unwrap(),
        AsrRecognitionEvent::revised_hypothesis(
            None,
            LanguageTextRange::new(6, 5).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            ", ".into(),
        )
        .unwrap(),
        AsrRecognitionEvent::revised_hypothesis(
            None,
            LanguageTextRange::new(7, 7).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            "Travis ".into(),
        )
        .unwrap(),
        AsrRecognitionEvent::committed_segment(
            None,
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            None,
            "Hello, Travis.".into(),
            BoundedSequence::new(),
        )
        .unwrap(),
    ];
    let envelopes = events
        .into_iter()
        .enumerate()
        .map(|(sequence, event)| {
            let time = ListeningEventTime::new(sequence as i64, ListeningClockOrigin::StreamStart)
                .unwrap();
            AsrRecognitionEnvelope::new(
                event,
                ListeningEventId::new(format!("asr/word-stream/{sequence}")).unwrap(),
                ListeningProvenance::new(
                    BoundedSequence::new(),
                    ListeningProvenanceKind::Direct,
                    None,
                    None,
                    BoundedSequence::new(),
                )
                .unwrap(),
                sequence as u64,
                stream.clone(),
                ListeningEventTimes::new(time.clone(), time).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "checked-asr-word-stream-bridge".into(),
        "fixture@1".into(),
    )
    .unwrap();
    // Each prepared revision borrows its predecessor. Keep the fixed chain in
    // distinct bindings so no vector growth can invalidate those references.
    let first = prepare(&basis, &envelopes[0], None, 0, &provenance);
    let second = prepare(&basis, &envelopes[1], Some(&first), 1, &provenance);
    let third = prepare(&basis, &envelopes[2], Some(&second), 2, &provenance);
    let fourth = prepare(&basis, &envelopes[3], Some(&third), 3, &provenance);
    let prepared = [&first, &second, &third, &fourth];
    for (sequence, next) in prepared.iter().enumerate() {
        assert_eq!(next.envelope(), &envelopes[sequence]);
        assert_eq!(
            next.revision().material().text(),
            ["Hello ", "Hello, ", "Hello, Travis ", "Hello, Travis."][sequence]
        );
        assert_eq!(*next.revision().sequence(), sequence as u64);
        assert_eq!(
            *next.revision().finality(),
            if sequence == 3 {
                LanguageTextFinality::Final
            } else {
                LanguageTextFinality::Partial
            }
        );
    }
    let receipt = serde_json::json!({
        "schema":"language/asr-word-stream-revisions@1",
        "scope":"checked supplied recognition events through existing ASR revision bridge; parser is a separate actual run",
        "asr_envelope_bytes":envelopes.iter().map(|v| v.clone().encode().unwrap()).collect::<Vec<_>>(),
        "language_revision_bytes":prepared.iter().map(|v| v.revision().clone().encode().unwrap()).collect::<Vec<_>>(),
        "provider_accuracy":false,
        "playback_committed_scalars":0,
    });
    let output = std::env::var("CONDUIT_ASR_WORD_STREAM_OUTPUT").unwrap();
    std::fs::write(output, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
}

fn prepare<'a>(
    basis: &AsrLanguageBasis<'a>,
    envelope: &'a AsrRecognitionEnvelope,
    previous: Option<&'a PreparedAsrRevision<'a>>,
    sequence: usize,
    provenance: &LinguisticDerivationProvenance,
) -> PreparedAsrRevision<'a> {
    let change = prepare_asr_revision(
        basis,
        envelope,
        previous,
        LanguageTextRevisionId::new(format!("independent-vocative/{sequence}")).unwrap(),
        provenance.clone(),
        Some([5, 6, 13, 14][sequence]),
        0,
        64,
    )
    .unwrap();
    let PreparedAsrChange::Revision(next) = change else {
        panic!("revision required")
    };
    next
}
