#![cfg(feature = "form-catalog")]

use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_net::*;

#[test]
fn local_queue_partial_and_remote_receipt_are_distinct_exact_states() {
    let mut delivery = RecordDeliveryTracker::locally_accepted(b"message/7", 100).unwrap();
    let capacities = delivery.allocated_capacities();
    assert_eq!(delivery.correlation(), b"message/7");
    assert_eq!(delivery.state(), RecordDeliveryStateRef::LocallyAccepted);
    assert!(!delivery.is_terminal());

    delivery.framed_queued(19).unwrap();
    assert_eq!(
        delivery.state(),
        RecordDeliveryStateRef::FramedQueued { queue_sequence: 19 }
    );
    delivery.partially_sent(40).unwrap();
    assert_eq!(
        delivery.state(),
        RecordDeliveryStateRef::PartiallySent {
            sent_bytes: 40,
            frame_bytes: 100,
        }
    );
    delivery.remote_accepted(b"receipt/remote/22").unwrap();
    assert_eq!(
        delivery.state(),
        RecordDeliveryStateRef::RemoteAccepted {
            receipt: b"receipt/remote/22",
        }
    );
    assert!(delivery.is_terminal());
    assert_eq!(delivery.allocated_capacities(), capacities);
}

#[test]
fn invalid_or_regressing_progress_and_unproved_receipt_refuse() {
    let mut delivery = RecordDeliveryTracker::locally_accepted(b"message/8", 20).unwrap();
    assert_eq!(
        delivery.partially_sent(1),
        Err(RecordDeliveryRefusal::InvalidTransition)
    );
    delivery.framed_queued(3).unwrap();
    for invalid in [0, 20, 21] {
        assert_eq!(
            delivery.partially_sent(invalid),
            Err(RecordDeliveryRefusal::InvalidPartialProgress)
        );
    }
    delivery.partially_sent(10).unwrap();
    assert_eq!(
        delivery.partially_sent(9),
        Err(RecordDeliveryRefusal::InvalidPartialProgress)
    );
    assert_eq!(
        delivery.remote_accepted(b""),
        Err(RecordDeliveryRefusal::EmptyReceipt)
    );
    assert!(!delivery.is_terminal());
}

#[test]
fn unavailable_disconnect_timeout_refusal_and_failure_remain_distinct_terminal_truth() {
    let cases = [
        (
            RecordDeliveryTracker::transport_unavailable as fn(&mut _, _) -> _,
            RecordDeliveryStateRef::TransportUnavailable { code: 11 },
        ),
        (
            RecordDeliveryTracker::disconnected,
            RecordDeliveryStateRef::Disconnected { code: 11 },
        ),
        (
            RecordDeliveryTracker::timed_out,
            RecordDeliveryStateRef::TimedOut { code: 11 },
        ),
        (
            RecordDeliveryTracker::refused,
            RecordDeliveryStateRef::Refused { code: 11 },
        ),
        (
            RecordDeliveryTracker::failed,
            RecordDeliveryStateRef::Failed { code: 11 },
        ),
    ];
    for (terminate, expected) in cases {
        let mut delivery = RecordDeliveryTracker::locally_accepted(b"correlation", 10).unwrap();
        terminate(&mut delivery, 11).unwrap();
        assert_eq!(delivery.state(), expected);
        assert!(delivery.is_terminal());
        assert_eq!(
            delivery.framed_queued(0),
            Err(RecordDeliveryRefusal::InvalidTransition)
        );
    }
}

#[test]
fn correlation_frame_receipt_and_transition_bounds_fail_closed() {
    assert_eq!(
        RecordDeliveryTracker::locally_accepted(b"", 1),
        Err(RecordDeliveryRefusal::EmptyCorrelation)
    );
    assert_eq!(
        RecordDeliveryTracker::locally_accepted(&[b'x'; MAXIMUM_RECORD_CORRELATION_BYTES + 1], 1),
        Err(RecordDeliveryRefusal::CorrelationTooLong)
    );
    assert_eq!(
        RecordDeliveryTracker::locally_accepted(b"x", 0),
        Err(RecordDeliveryRefusal::EmptyFrame)
    );
    assert_eq!(
        RecordDeliveryTracker::locally_accepted(b"x", MAXIMUM_TYPED_RECORD_FRAME_BYTES + 1),
        Err(RecordDeliveryRefusal::FrameTooLarge)
    );
    let mut delivery = RecordDeliveryTracker::locally_accepted(b"x", 2).unwrap();
    delivery.framed_queued(0).unwrap();
    assert_eq!(
        delivery.remote_accepted(&[b'r'; MAXIMUM_RECORD_RECEIPT_BYTES + 1]),
        Err(RecordDeliveryRefusal::ReceiptTooLong)
    );
}

#[test]
fn every_delivery_observation_has_an_exact_bounded_wire_round_trip() {
    fn round_trip(event: RecordDeliveryObservationEvent<'_>) {
        let observation = RecordDeliveryObservationRef {
            correlation: b"message/9",
            frame_bytes: 100,
            event,
        };
        let mut wire = [0_u8; MAXIMUM_RECORD_DELIVERY_WIRE_BYTES];
        let written = encode_record_delivery_observation_into(observation, &mut wire).unwrap();
        assert_eq!(
            decode_record_delivery_observation(&wire[..written]).unwrap(),
            observation
        );
    }

    round_trip(RecordDeliveryObservationEvent::LocallyAccepted);
    round_trip(RecordDeliveryObservationEvent::FramedQueued { queue_sequence: 41 });
    round_trip(RecordDeliveryObservationEvent::PartiallySent { sent_bytes: 29 });
    round_trip(RecordDeliveryObservationEvent::RemoteAccepted {
        receipt: b"remote/receipt/4",
    });
    round_trip(RecordDeliveryObservationEvent::TransportUnavailable { code: 1 });
    round_trip(RecordDeliveryObservationEvent::Disconnected { code: 2 });
    round_trip(RecordDeliveryObservationEvent::TimedOut { code: 3 });
    round_trip(RecordDeliveryObservationEvent::Refused { code: 4 });
    round_trip(RecordDeliveryObservationEvent::Failed { code: 5 });
}

#[test]
fn delivery_wire_refuses_truncation_trailing_bytes_empty_receipts_and_small_output() {
    let observation = RecordDeliveryObservationRef {
        correlation: b"message/10",
        frame_bytes: 20,
        event: RecordDeliveryObservationEvent::FramedQueued { queue_sequence: 7 },
    };
    let mut wire = [0_u8; MAXIMUM_RECORD_DELIVERY_WIRE_BYTES];
    let written = encode_record_delivery_observation_into(observation, &mut wire).unwrap();
    for length in 0..written {
        assert!(decode_record_delivery_observation(&wire[..length]).is_err());
    }
    wire[written] = 99;
    assert_eq!(
        decode_record_delivery_observation(&wire[..written + 1]),
        Err(RecordDeliveryRefusal::MalformedWire)
    );
    assert_eq!(
        encode_record_delivery_observation_into(observation, &mut [0_u8; 1]),
        Err(RecordDeliveryRefusal::OutputTooSmall)
    );

    let empty_receipt = RecordDeliveryObservationRef {
        event: RecordDeliveryObservationEvent::RemoteAccepted { receipt: b"" },
        ..observation
    };
    assert_eq!(
        encode_record_delivery_observation_into(empty_receipt, &mut wire),
        Err(RecordDeliveryRefusal::EmptyReceipt)
    );
    let mut malformed_receipt = [0_u8; 9];
    malformed_receipt[0] = RECORD_DELIVERY_WIRE_VERSION;
    malformed_receipt[1] = 1;
    malformed_receipt[2..6].copy_from_slice(&1_u32.to_le_bytes());
    malformed_receipt[6] = 3;
    malformed_receipt[7] = b'x';
    assert_eq!(
        decode_record_delivery_observation(&malformed_receipt),
        Err(RecordDeliveryRefusal::MalformedWire)
    );
}

#[test]
fn tracker_applies_only_matching_ordered_observations_and_projects_exact_status() {
    let mut delivery = RecordDeliveryTracker::locally_accepted(b"message/11", 50).unwrap();
    let queued = RecordDeliveryObservationRef {
        correlation: b"message/11",
        frame_bytes: 50,
        event: RecordDeliveryObservationEvent::FramedQueued { queue_sequence: 8 },
    };
    delivery.apply(queued).unwrap();
    assert_eq!(delivery.observation(), queued);

    let wrong_identity = RecordDeliveryObservationRef {
        correlation: b"message/12",
        ..queued
    };
    assert_eq!(
        delivery.apply(wrong_identity),
        Err(RecordDeliveryRefusal::ObservationIdentityMismatch)
    );
    delivery
        .apply(RecordDeliveryObservationRef {
            event: RecordDeliveryObservationEvent::PartiallySent { sent_bytes: 12 },
            ..queued
        })
        .unwrap();
    delivery
        .apply(RecordDeliveryObservationRef {
            event: RecordDeliveryObservationEvent::RemoteAccepted {
                receipt: b"remote/12",
            },
            ..queued
        })
        .unwrap();
    assert_eq!(
        delivery.observation().event,
        RecordDeliveryObservationEvent::RemoteAccepted {
            receipt: b"remote/12",
        }
    );
}

#[test]
fn delivery_projection_is_an_ordinary_reusable_closing_flow_form() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_record_delivery_status_catalog(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/record-delivery-status/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "record-delivery-status", &profile).unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        RECORD_DELIVERY_STATUS_KIND
    );
}
