//! Explicit checked ASR events, independent of provider recognition accuracy.
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{language_revision::*, semantic::*};
use serde_json::Value;
fn envelope(
    stream: &ListeningStreamId,
    sequence: u64,
    event: AsrRecognitionEvent,
) -> AsrRecognitionEnvelope {
    let time = ListeningEventTime::new(
        i64::try_from(sequence).unwrap(),
        ListeningClockOrigin::StreamStart,
    )
    .unwrap();
    AsrRecognitionEnvelope::new(
        event,
        ListeningEventId::new(format!("asr/graph/event/{sequence}")).unwrap(),
        ListeningProvenance::new(
            BoundedSequence::new(),
            ListeningProvenanceKind::Direct,
            None,
            None,
            BoundedSequence::new(),
        )
        .unwrap(),
        sequence,
        stream.clone(),
        ListeningEventTimes::new(time.clone(), time).unwrap(),
    )
    .unwrap()
}
fn revision(change: PreparedAsrChange<'_>) -> PreparedAsrRevision<'_> {
    match change {
        PreparedAsrChange::Revision(value) => value,
        _ => panic!("revision expected"),
    }
}
pub fn prepare(id: &str, native: &LanguageLexicalTape) -> Value {
    let stream = ListeningStreamId::new(format!("asr/graph/{id}")).unwrap();
    let segment = ListeningSegmentId::new(format!("asr/graph/{id}")).unwrap();
    let text = LanguageTextId::new(format!("asr/graph/{id}")).unwrap();
    let language = native.source().material().language();
    let basis = AsrLanguageBasis {
        stream: &stream,
        segment: &segment,
        text: &text,
        language,
    };
    let content = native.source().material().text();
    let prefix = content.chars().take(1).collect::<String>();
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "checked-asr-graph-bridge".into(),
        "fixture@1".into(),
    )
    .unwrap();
    let partial = envelope(
        &stream,
        0,
        AsrRecognitionEvent::partial_hypothesis(
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            prefix,
        )
        .unwrap(),
    );
    let first = revision(
        prepare_asr_revision(
            &basis,
            &partial,
            None,
            LanguageTextRevisionId::new(format!("asr/{id}/r0")).unwrap(),
            provenance.clone(),
            Some(0),
            0,
            4096,
        )
        .unwrap(),
    );
    let first_tape = prepare_lexical_tape(first.revision(), native.profile(), None).unwrap();
    let revised = envelope(
        &stream,
        1,
        AsrRecognitionEvent::revised_hypothesis(
            None,
            LanguageTextRange::new(1, 0).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            content.clone(),
        )
        .unwrap(),
    );
    let second = revision(
        prepare_asr_revision(
            &basis,
            &revised,
            Some(&first),
            LanguageTextRevisionId::new(format!("asr/{id}/r1")).unwrap(),
            provenance.clone(),
            Some(0),
            0,
            4096,
        )
        .unwrap(),
    );
    let second_tape =
        prepare_lexical_tape(second.revision(), native.profile(), Some(&first_tape)).unwrap();
    let committed = envelope(
        &stream,
        2,
        AsrRecognitionEvent::committed_segment(
            None,
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            None,
            content.clone(),
            BoundedSequence::new(),
        )
        .unwrap(),
    );
    let final_revision = revision(
        prepare_asr_revision(
            &basis,
            &committed,
            Some(&second),
            LanguageTextRevisionId::new(format!("asr/{id}/r2")).unwrap(),
            provenance,
            Some(0),
            0,
            4096,
        )
        .unwrap(),
    );
    let final_tape = prepare_lexical_tape(
        final_revision.revision(),
        native.profile(),
        Some(&second_tape),
    )
    .unwrap();
    assert_eq!(final_tape.tape().source().material().text(), content);
    assert_eq!(*final_tape.tape().source().sequence(), 2);
    assert_eq!(first.envelope(), &partial);
    assert_eq!(second.envelope(), &revised);
    assert_eq!(final_revision.envelope(), &committed);
    serde_json::json!({"id":id,"forms":final_tape.tape().tokens().iter().map(|t|t.surface()).collect::<Vec<_>>(),"lexical_tape_bytes":final_tape.tape().clone().encode().unwrap(),"asr_envelope_bytes":[partial.clone().encode().unwrap(),revised.clone().encode().unwrap(),committed.clone().encode().unwrap()],"language_revision_bytes":[first.revision().clone().encode().unwrap(),second.revision().clone().encode().unwrap(),final_revision.revision().clone().encode().unwrap()],"provider_accuracy":false,"playback_committed_scalars":0})
}

pub fn admit_history(
    row: &Value,
    native: &LanguageLexicalTape,
) -> Result<PreparedLexicalTape, String> {
    fn decode<T: NativeRustBinding>(value: &Value) -> Result<T, String> {
        let bytes = value.as_array().ok_or("native bytes")?;
        if bytes.is_empty() || bytes.len() > 262144 {
            return Err("native byte bound".into());
        }
        let bytes = bytes
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()).ok_or("byte"))
            .collect::<Result<Vec<_>, _>>()?;
        T::decode(&bytes).map_err(|e| format!("native: {e:?}"))
    }
    let events = row["asr_envelope_bytes"].as_array().ok_or("ASR events")?;
    let revisions = row["language_revision_bytes"]
        .as_array()
        .ok_or("ASR revisions")?;
    if events.len() != 3 || revisions.len() != 3 {
        return Err("ASR chain bound".into());
    }
    let events = events
        .iter()
        .map(decode::<AsrRecognitionEnvelope>)
        .collect::<Result<Vec<_>, _>>()?;
    let sources = revisions
        .iter()
        .map(decode::<LanguageTextRevision>)
        .collect::<Result<Vec<_>, _>>()?;
    let AsrRecognitionEvent::PartialHypothesis(first_event) = events[0].event() else {
        return Err("first partial required".into());
    };
    if !matches!(events[1].event(), AsrRecognitionEvent::RevisedHypothesis(_))
        || !matches!(events[2].event(), AsrRecognitionEvent::CommittedSegment(_))
    {
        return Err("revised/committed required".into());
    }
    let basis = AsrLanguageBasis {
        stream: events[0].stream_id(),
        segment: first_event.segment_id(),
        text: sources[0].material().identity(),
        language: sources[0].material().language(),
    };
    fn checked<'a>(
        basis: &AsrLanguageBasis<'a>,
        event: &'a AsrRecognitionEnvelope,
        expected: &LanguageTextRevision,
        previous: Option<&'a PreparedAsrRevision<'a>>,
    ) -> Result<PreparedAsrRevision<'a>, String> {
        let change = prepare_asr_revision(
            basis,
            event,
            previous,
            expected.material().revision().clone(),
            expected.provenance().clone(),
            *expected.stable_prefix(),
            0,
            4096,
        )
        .map_err(|e| format!("ASR bridge: {e:?}"))?;
        let PreparedAsrChange::Revision(next) = change else {
            return Err("ASR cancellation".into());
        };
        if next.revision() != expected {
            return Err("foreign ASR-derived revision".into());
        }
        Ok(next)
    }
    let first = checked(&basis, &events[0], &sources[0], None)?;
    let second = checked(&basis, &events[1], &sources[1], Some(&first))?;
    let final_revision = checked(&basis, &events[2], &sources[2], Some(&second))?;
    let first_tape = prepare_lexical_tape(first.revision(), native.profile(), None)
        .map_err(|e| format!("ASR lexical: {e:?}"))?;
    let second_tape = prepare_lexical_tape(second.revision(), native.profile(), Some(&first_tape))
        .map_err(|e| format!("ASR lexical: {e:?}"))?;
    let lexical = prepare_lexical_tape(
        final_revision.revision(),
        native.profile(),
        Some(&second_tape),
    )
    .map_err(|e| format!("ASR lexical: {e:?}"))?;
    if lexical.tape() != native {
        return Err("foreign ASR final tape".into());
    }
    Ok(lexical)
}
