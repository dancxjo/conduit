use super::*;
use crate::{SessionRole, SessionTerminalDisposition};
use ExternalForeSessionMessage::{Hello, Message};
fn binding() -> ExternalForeSessionBinding {
    ExternalForeSessionBinding {
        scope_commitment: [7; 32],
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 16,
            maximum_buffered_bytes: 16,
        },
        abnormal_kind: None,
        maximum_frame_bytes: 128,
    }
}
fn pair() -> (ExternalForeSessionMachine, ExternalForeSessionMachine) {
    (
        ExternalForeSessionMachine::new(binding(), SessionRole::Source).unwrap(),
        ExternalForeSessionMachine::new(binding(), SessionRole::Sink).unwrap(),
    )
}
fn exchange(
    source: &mut ExternalForeSessionMachine,
    sink: &mut ExternalForeSessionMachine,
    message: ExternalForeSessionMessage<'_>,
) {
    let frame = source.binding().frame(message);
    let mut bytes = [0; 128];
    let length = encode_external_fore_session_frame_into(frame, &mut bytes, 16, 128).unwrap();
    let decoded = decode_external_fore_session_frame(&bytes[..length], 16, 128).unwrap();
    assert_eq!(decoded, frame);
    source.admit_outbound(frame).unwrap();
    sink.admit_inbound(decoded).unwrap();
}
fn active(source: &mut ExternalForeSessionMachine, sink: &mut ExternalForeSessionMachine) {
    exchange(source, sink, Hello);
    exchange(sink, source, Hello);
    exchange(source, sink, Message(SessionMessage::Ready));
    exchange(sink, source, Message(SessionMessage::Ready));
    assert!(source.is_active() && sink.is_active());
}
#[test]
fn pressure_reoffer_accept_delivery_close_and_terminal_use_shared_lifecycle() {
    let (mut source, mut sink) = pair();
    active(&mut source, &mut sink);
    let offer = Message(SessionMessage::Offered {
        sequence: 0,
        payload: b"index",
    });
    exchange(&mut source, &mut sink, offer);
    exchange(
        &mut sink,
        &mut source,
        Message(SessionMessage::Pressure { sequence: 0 }),
    );
    assert_eq!(source.next_sequence(), 0);
    exchange(&mut source, &mut sink, offer);
    exchange(
        &mut sink,
        &mut source,
        Message(SessionMessage::Accepted { sequence: 0 }),
    );
    assert_eq!(
        source.next_sequence(),
        0,
        "acceptance is not delivery or durable commit"
    );
    exchange(
        &mut sink,
        &mut source,
        Message(SessionMessage::Delivered { sequence: 0 }),
    );
    assert_eq!(source.next_sequence(), 1);
    exchange(
        &mut source,
        &mut sink,
        Message(SessionMessage::InputClosed { final_sequence: 1 }),
    );
    let terminal = Message(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence: 1,
    });
    exchange(&mut source, &mut sink, terminal);
    exchange(&mut sink, &mut source, terminal);
    assert!(source.is_terminal() && sink.is_terminal());
    assert_eq!(
        source.admit_outbound(source.binding().frame(Hello)),
        Err(WireError::LateFrame)
    );
}
#[test]
fn exact_scope_readiness_duplicates_direction_and_bounds_refuse_before_progress() {
    let (mut source, mut sink) = pair();
    let frame = source.binding().frame(Message(SessionMessage::Ready));
    assert_eq!(source.admit_outbound(frame), Err(WireError::InvalidState));
    let mut wrong = source.binding().frame(Hello);
    wrong.scope_commitment[0] ^= 1;
    assert_eq!(sink.admit_inbound(wrong), Err(WireError::InvalidSession));
    active(&mut source, &mut sink);
    assert_eq!(
        sink.admit_outbound(sink.binding().frame(Message(SessionMessage::Offered {
            sequence: 0,
            payload: b"x"
        }))),
        Err(WireError::ReorderedFrame)
    );
    assert_eq!(
        source.admit_outbound(source.binding().frame(Message(SessionMessage::Offered {
            sequence: 0,
            payload: &[0; 17]
        }))),
        Err(WireError::OversizedPayload)
    );
    let offered = source.binding().frame(Message(SessionMessage::Offered {
        sequence: 0,
        payload: b"x",
    }));
    source.admit_outbound(offered).unwrap();
    assert_eq!(
        source.admit_outbound(offered),
        Err(WireError::DuplicateFrame)
    );
    assert_eq!(
        source.admit_inbound(
            source
                .binding()
                .frame(Message(SessionMessage::Delivered { sequence: 0 }))
        ),
        Err(WireError::ReorderedFrame)
    );
    assert_eq!(source.next_sequence(), 0);
    let mut invalid = binding();
    invalid.scope_commitment = [0; 32];
    assert_eq!(invalid.validate(), Err(WireError::InvalidSession));
}
#[test]
fn cancellation_and_failure_keep_terminal_dispositions_distinct() {
    for cancelled in [true, false] {
        let (mut source, mut sink) = pair();
        active(&mut source, &mut sink);
        exchange(
            &mut source,
            &mut sink,
            Message(SessionMessage::Offered {
                sequence: 0,
                payload: b"x",
            }),
        );
        let (failure, disposition) = if cancelled {
            (
                SessionMessage::Cancelled { code: 1 },
                SessionTerminalDisposition::Cancelled,
            )
        } else {
            (
                SessionMessage::Failed { code: 2 },
                SessionTerminalDisposition::Failed,
            )
        };
        exchange(&mut source, &mut sink, Message(failure));
        exchange(&mut sink, &mut source, Message(failure));
        let terminal = Message(SessionMessage::Terminal {
            disposition,
            final_sequence: 0,
        });
        exchange(&mut source, &mut sink, terminal);
        exchange(&mut sink, &mut source, terminal);
        assert!(source.is_terminal() && sink.is_terminal());
    }
}
#[test]
fn codec_rejects_wrong_versions_truncation_trailing_and_payload_pressure() {
    let frame = binding().frame(Message(SessionMessage::Offered {
        sequence: 0,
        payload: b"result",
    }));
    let mut bytes = [0; 128];
    let length = encode_external_fore_session_frame_into(frame, &mut bytes, 16, 128).unwrap();
    for end in 0..length {
        assert!(decode_external_fore_session_frame(&bytes[..end], 16, 128).is_err());
    }
    assert_eq!(
        decode_external_fore_session_frame(&bytes[..length + 1], 16, 128),
        Err(WireError::TrailingGarbage)
    );
    assert_eq!(
        decode_external_fore_session_frame(&bytes[..length], 1, 128),
        Err(WireError::OversizedPayload)
    );
    bytes[4] = 2;
    assert_eq!(
        decode_external_fore_session_frame(&bytes[..length], 16, 128),
        Err(WireError::UnsupportedWireFormat)
    );
}
