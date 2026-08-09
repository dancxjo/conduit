use conduit_core::{BootId, ConnectionBase};
use conduit_signal::SIGNAL_ENCODED_LEN;
use conduit_std_host::triple_signal::{RemoteKind, TripleSource};
use conduit_wire::{
    decode_session_frame, SessionFrame, SessionMachine, SessionMessage, SessionRole,
    SessionTerminalDisposition, WireError,
};

fn trigger(source: &mut TripleSource, kind: RemoteKind, sink: &mut SessionMachine) {
    let binding = source.binding(kind).clone();
    let hello = binding.hello_frame();
    source.admit_outbound(kind, hello).unwrap();
    sink.admit_inbound(hello).unwrap();
    sink.admit_outbound(hello).unwrap();
    source.admit_inbound(kind, hello).unwrap();
    let ready = binding.frame(SessionMessage::Ready);
    source.admit_outbound(kind, ready).unwrap();
    sink.admit_inbound(ready).unwrap();
    sink.admit_outbound(ready).unwrap();
    source.admit_inbound(kind, ready).unwrap();
    assert!(source.is_active(kind));
}

#[test]
fn one_kernel_atomically_fans_each_value_to_stdout_browser_and_pico() {
    let mut source = TripleSource::prepare().expect("triple source prepares");
    let mut browser = SessionMachine::new(
        source.binding(RemoteKind::Browser).clone(),
        SessionRole::Sink,
    )
    .unwrap();
    let mut pico =
        SessionMachine::new(source.binding(RemoteKind::Pico).clone(), SessionRole::Sink).unwrap();
    trigger(&mut source, RemoteKind::Browser, &mut browser);
    trigger(&mut source, RemoteKind::Pico, &mut pico);

    let mut sequences = Vec::new();
    while let Some(offer) = source.next_offer().unwrap() {
        assert_eq!(offer.payload.len(), SIGNAL_ENCODED_LEN as usize);
        sequences.push(offer.sequence);
        for (kind, sink) in [
            (RemoteKind::Browser, &mut browser),
            (RemoteKind::Pico, &mut pico),
        ] {
            let binding = source.binding(kind).clone();
            let offered = binding.frame(SessionMessage::Offered {
                sequence: offer.sequence,
                payload: &offer.payload,
            });
            source.admit_outbound(kind, offered).unwrap();
            sink.admit_inbound(offered).unwrap();
            let accepted = binding.frame(SessionMessage::Accepted {
                sequence: offer.sequence,
            });
            sink.admit_outbound(accepted).unwrap();
            source.admit_inbound(kind, accepted).unwrap();
            source.accepted(kind, offer.sequence).unwrap();
            let delivered = binding.frame(SessionMessage::Delivered {
                sequence: offer.sequence,
            });
            sink.admit_outbound(delivered).unwrap();
            source.admit_inbound(kind, delivered).unwrap();
            source.delivered(kind, offer.sequence).unwrap();
        }
    }
    assert_eq!(sequences, (0..16).collect::<Vec<_>>());
    assert_eq!(source.finish_kernel().unwrap(), 16);
    assert_eq!(
        source
            .receipts()
            .iter()
            .map(|receipt| receipt.sequence)
            .collect::<Vec<_>>(),
        sequences
    );
    assert!(source
        .receipts()
        .iter()
        .enumerate()
        .all(|(index, receipt)| receipt.level == (index % 2 == 1)));

    for (kind, sink) in [
        (RemoteKind::Browser, &mut browser),
        (RemoteKind::Pico, &mut pico),
    ] {
        let binding = source.binding(kind).clone();
        let closed = binding.frame(SessionMessage::InputClosed { final_sequence: 16 });
        source.admit_outbound(kind, closed).unwrap();
        sink.admit_inbound(closed).unwrap();
        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: 16,
        });
        source.admit_outbound(kind, terminal).unwrap();
        sink.admit_inbound(terminal).unwrap();
        sink.admit_outbound(terminal).unwrap();
        source.admit_inbound(kind, terminal).unwrap();
        assert!(source.is_terminal(kind));
    }
}

#[test]
fn stale_pico_boot_and_base_instance_fail_before_play_start() {
    let mut source = TripleSource::prepare().expect("triple source prepares");
    let planned = source.binding(RemoteKind::Pico).clone();
    source
        .observe_pico_boot(BootId::from("observed-triple-pico-boot"))
        .unwrap();
    assert_eq!(
        source.admit_inbound(RemoteKind::Pico, planned.hello_frame()),
        Err(format!("{:?}", WireError::InvalidSession))
    );

    let binding = source.binding(RemoteKind::Pico).clone();
    assert_eq!(binding.attachment.base, ConnectionBase::UsbCdc);
    let mut message = binding.hello_frame().message;
    if let SessionMessage::Hello(ref mut hello) = message {
        hello.base_instance_id = "wrong-triple-base";
    }
    assert_eq!(
        source.admit_inbound(
            RemoteKind::Pico,
            SessionFrame {
                identity: binding.identity(),
                message,
            },
        ),
        Err(format!("{:?}", WireError::InvalidSession))
    );
}

#[test]
fn malformed_frame_is_rejected_and_sink_failure_reaches_both_exact_sessions() {
    assert!(decode_session_frame(&[0_u8; 8], SIGNAL_ENCODED_LEN, 2_048).is_err());

    let mut source = TripleSource::prepare().expect("triple source prepares");
    let mut browser = SessionMachine::new(
        source.binding(RemoteKind::Browser).clone(),
        SessionRole::Sink,
    )
    .unwrap();
    let mut pico =
        SessionMachine::new(source.binding(RemoteKind::Pico).clone(), SessionRole::Sink).unwrap();
    trigger(&mut source, RemoteKind::Browser, &mut browser);
    trigger(&mut source, RemoteKind::Pico, &mut pico);
    source.cancel().expect("one kernel cancels");

    for (kind, sink) in [
        (RemoteKind::Browser, &mut browser),
        (RemoteKind::Pico, &mut pico),
    ] {
        let binding = source.binding(kind).clone();
        let failed = binding.frame(SessionMessage::Failed { code: 350 });
        source.admit_outbound(kind, failed).unwrap();
        sink.admit_inbound(failed).unwrap();
        sink.admit_outbound(failed).unwrap();
        source.admit_inbound(kind, failed).unwrap();
        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Failed,
            final_sequence: 0,
        });
        source.admit_outbound(kind, terminal).unwrap();
        sink.admit_inbound(terminal).unwrap();
        sink.admit_outbound(terminal).unwrap();
        source.admit_inbound(kind, terminal).unwrap();
        assert!(source.is_terminal(kind));
    }
}
