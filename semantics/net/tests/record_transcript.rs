#![cfg(feature = "form-catalog")]

use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_net::*;

fn frame(text: &[u8]) -> Vec<u8> {
    let value = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text@1")).unwrap(),
        text.to_vec(),
    )
    .unwrap();
    let profile = value.value_type().profile().unwrap();
    let value_kind = profile.value_kind().as_str();
    let payload = value.canonical_bytes().unwrap();
    let record = TypedRecordRef::new(value_kind, &payload).unwrap();
    let mut frame = vec![0_u8; MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let length = encode_typed_record_into(record, &mut frame).unwrap();
    frame.truncate(length);
    frame
}

#[test]
fn sent_received_and_terminal_events_retain_exact_identity_and_order() {
    let sent = frame(b"outbound");
    let received = frame(b"inbound");
    let maximum = sent.len().max(received.len());
    let mut history = BoundedRecordTranscript::new(3, maximum, maximum * 2, 41).unwrap();

    assert_eq!(
        history.record(RecordTranscriptDirection::Sent, &sent),
        Ok(41)
    );
    assert_eq!(
        history.record(RecordTranscriptDirection::Received, &received),
        Ok(42)
    );
    assert_eq!(
        history.terminal(RecordTranscriptTerminal::Disconnected),
        Ok(43)
    );
    assert_eq!(history.len(), 3);
    assert_eq!(history.retained_bytes(), sent.len() + received.len());
    assert_eq!(history.retention_gap(), 0);
    assert_eq!(history.entry(0).unwrap().sequence, 41);
    assert_eq!(
        history.entry(0).unwrap().event,
        RecordTranscriptEventRef::Record {
            direction: RecordTranscriptDirection::Sent,
            frame: &sent,
        }
    );
    assert_eq!(
        history.entry(1).unwrap().event,
        RecordTranscriptEventRef::Record {
            direction: RecordTranscriptDirection::Received,
            frame: &received,
        }
    );
    assert_eq!(
        history.entry(2).unwrap().event,
        RecordTranscriptEventRef::Terminal(RecordTranscriptTerminal::Disconnected)
    );
}

#[test]
fn item_and_byte_pressure_evict_only_whole_oldest_events_and_report_the_gap() {
    let a = frame(b"aaaa");
    let b = frame(b"bbbb");
    let c = frame(b"cccc");
    let maximum = a.len();
    let mut history = BoundedRecordTranscript::new(2, maximum, maximum * 2, 0).unwrap();
    let capacities = [history.slot_capacity(0), history.slot_capacity(1)];
    history.record(RecordTranscriptDirection::Sent, &a).unwrap();
    history
        .record(RecordTranscriptDirection::Received, &b)
        .unwrap();
    history.record(RecordTranscriptDirection::Sent, &c).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history.retention_gap(), 1);
    assert_eq!(history.entry(0).unwrap().sequence, 1);
    assert_eq!(history.entry(1).unwrap().sequence, 2);
    assert_eq!(
        [history.slot_capacity(0), history.slot_capacity(1)],
        capacities
    );

    let small = frame(b"x");
    let large = frame(b"a larger retained event");
    let mut byte_limited = BoundedRecordTranscript::new(3, large.len(), large.len(), 8).unwrap();
    byte_limited
        .record(RecordTranscriptDirection::Sent, &small)
        .unwrap();
    byte_limited
        .record(RecordTranscriptDirection::Received, &large)
        .unwrap();
    assert_eq!(byte_limited.len(), 1);
    assert_eq!(byte_limited.retention_gap(), 1);
    assert_eq!(byte_limited.entry(0).unwrap().sequence, 9);
}

#[test]
fn invalid_limits_frames_and_exhausted_sequences_refuse_without_false_history() {
    let valid = frame(b"valid");
    assert_eq!(
        BoundedRecordTranscript::new(0, valid.len(), valid.len(), 0),
        Err(RecordTranscriptRefusal::InvalidLimits)
    );
    let mut history = BoundedRecordTranscript::new(1, valid.len(), valid.len(), 0).unwrap();
    assert_eq!(
        history.record(RecordTranscriptDirection::Sent, b"bad"),
        Err(RecordTranscriptRefusal::InvalidFrame(
            TypedRecordFrameRefusal::Truncated
        ))
    );
    assert!(history.is_empty());

    let mut too_small = BoundedRecordTranscript::new(1, valid.len() - 1, valid.len(), 0).unwrap();
    assert_eq!(
        too_small.record(RecordTranscriptDirection::Sent, &valid),
        Err(RecordTranscriptRefusal::FrameTooLarge)
    );

    let mut exhausted =
        BoundedRecordTranscript::new(1, valid.len(), valid.len(), u64::MAX).unwrap();
    assert_eq!(
        exhausted.terminal(RecordTranscriptTerminal::Completed),
        Err(RecordTranscriptRefusal::SequenceExhausted)
    );
    assert!(exhausted.is_empty());
}

#[test]
fn transcript_is_an_ordinary_reusable_closing_flow_form() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    install_record_transcript_catalog(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/bounded-record-transcript/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "bounded-record-transcript", &profile)
            .unwrap();
    assert_eq!(authored.input_bindings.len(), 3);
    assert_eq!(authored.output_bindings.len(), 3);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        RECORD_TRANSCRIPT_KIND
    );
    assert!(authored.expanded.gears[0]
        .inputs
        .iter()
        .all(|input| input.temporal == conduit_core::PortTemporal::Flow { closes: true }));
}
