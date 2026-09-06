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
    let value_kind = value
        .value_type()
        .profile()
        .unwrap()
        .value_kind()
        .as_str()
        .to_string();
    let payload = value.canonical_bytes().unwrap();
    let record = TypedRecordRef::new(&value_kind, &payload).unwrap();
    let mut frame = vec![0_u8; MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let length = encode_typed_record_into(record, &mut frame).unwrap();
    frame.truncate(length);
    frame
}

#[test]
fn fifo_sequence_and_slot_capacities_remain_exact_across_wraparound() {
    let first = frame(b"first");
    let second = frame(b"second");
    let third = frame(b"third");
    let maximum = first.len().max(second.len()).max(third.len());
    let mut queue = BoundedOrderedRecordQueue::new(2, maximum, 7).unwrap();
    let capacities = [queue.slot_capacity(0), queue.slot_capacity(1)];
    assert_eq!(queue.enqueue(&first), Ok(7));
    assert_eq!(queue.enqueue(&second), Ok(8));
    let sent = queue.dequeue().unwrap();
    assert_eq!(sent.sequence, 7);
    assert_eq!(
        decode_typed_record(sent.frame).unwrap().payload(),
        decode_typed_record(&first).unwrap().payload()
    );
    assert_eq!(queue.enqueue(&third), Ok(9));
    assert_eq!(queue.dequeue().unwrap().sequence, 8);
    assert_eq!(queue.dequeue().unwrap().sequence, 9);
    assert_eq!([queue.slot_capacity(0), queue.slot_capacity(1)], capacities);
}

#[test]
fn full_malformed_oversize_closed_and_sequence_exhaustion_are_distinct() {
    let valid = frame(b"one");
    let mut full = BoundedOrderedRecordQueue::new(1, valid.len(), 0).unwrap();
    full.enqueue(&valid).unwrap();
    assert_eq!(full.enqueue(&valid), Err(OrderedRecordQueueRefusal::Full));
    assert_eq!(full.len(), 1);

    let mut malformed = BoundedOrderedRecordQueue::new(1, valid.len(), 0).unwrap();
    assert_eq!(
        malformed.enqueue(b"not-a-frame"),
        Err(OrderedRecordQueueRefusal::InvalidFrame(
            TypedRecordFrameRefusal::Truncated
        ))
    );
    let mut oversize = BoundedOrderedRecordQueue::new(1, valid.len() - 1, 0).unwrap();
    assert_eq!(
        oversize.enqueue(&valid),
        Err(OrderedRecordQueueRefusal::FrameTooLarge)
    );

    let mut closed = BoundedOrderedRecordQueue::new(1, valid.len(), 0).unwrap();
    closed.close_input();
    assert!(closed.is_terminal());
    assert_eq!(
        closed.enqueue(&valid),
        Err(OrderedRecordQueueRefusal::Closed)
    );

    let mut exhausted = BoundedOrderedRecordQueue::new(1, valid.len(), u64::MAX).unwrap();
    assert_eq!(
        exhausted.enqueue(&valid),
        Err(OrderedRecordQueueRefusal::SequenceExhausted)
    );
}

#[test]
fn unusable_item_and_frame_bounds_refuse_before_slot_allocation() {
    for result in [
        BoundedOrderedRecordQueue::new(0, TYPED_RECORD_FRAME_HEADER_BYTES, 0),
        BoundedOrderedRecordQueue::new(
            MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS + 1,
            TYPED_RECORD_FRAME_HEADER_BYTES,
            0,
        ),
        BoundedOrderedRecordQueue::new(1, TYPED_RECORD_FRAME_HEADER_BYTES - 1, 0),
        BoundedOrderedRecordQueue::new(1, MAXIMUM_TYPED_RECORD_FRAME_BYTES + 1, 0),
    ] {
        assert_eq!(result, Err(OrderedRecordQueueRefusal::InvalidLimits));
    }
}

#[test]
fn closing_preserves_queued_records_until_fifo_drain_then_becomes_terminal() {
    let valid = frame(b"retained");
    let mut queue = BoundedOrderedRecordQueue::new(2, valid.len(), 0).unwrap();
    queue.enqueue(&valid).unwrap();
    queue.close_input();
    assert!(queue.is_input_closed());
    assert!(!queue.is_terminal());
    assert!(queue.dequeue().is_some());
    assert!(queue.is_terminal());
}

#[test]
fn ordered_queue_is_a_reusable_closing_flow_form() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    install_ordered_record_queue_catalog(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/ordered-record-send-queue/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "ordered-record-send-queue", &profile)
            .unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    let queue = &authored.expanded.gears[0];
    assert_eq!(queue.kind_id.as_str(), ORDERED_RECORD_QUEUE_KIND);
    assert_eq!(
        queue.inputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        queue.outputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
}
