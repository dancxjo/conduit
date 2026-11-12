//! Pure framed peer fixtures; no membership, Start or physical equipment claim.
use super::*;
use conduit_wire::{SessionLimits, SessionMessage};
use std::collections::VecDeque;
fn bindings() -> [Binding; 2] {
    [1, 2].map(|tag| Binding {
        scope_commitment: [tag; 32],
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 16,
            maximum_buffered_bytes: 16,
        },
        abnormal_kind: None,
        maximum_frame_bytes: 128,
    })
}
fn bounds() -> ForeCarriageBounds {
    ForeCarriageBounds {
        maximum_frames: 40,
        maximum_payload_bytes: 16,
        maximum_frame_bytes: 128,
        maximum_buffer_bytes: 128,
    }
}
const TIMEOUT: Duration = Duration::from_secs(1);
struct Peer {
    machines: [Machine; 2],
    queue: VecDeque<Vec<u8>>,
    writes: usize,
    pressure: bool,
    fail_send: bool,
    fail_receive: bool,
    delay_second_hello: bool,
    reply: Option<&'static [u8]>,
    early_reply: bool,
    fail_send_at: Option<usize>,
}
impl Peer {
    fn new() -> Self {
        let [b0, b1] = bindings();
        Self {
            machines: [
                Machine::new(b0, SessionRole::Sink).unwrap(),
                Machine::new(b1, SessionRole::Source).unwrap(),
            ],
            queue: VecDeque::new(),
            writes: 0,
            pressure: false,
            fail_send: false,
            fail_receive: false,
            delay_second_hello: false,
            reply: None,
            early_reply: false,
            fail_send_at: None,
        }
    }
    fn emit(&mut self, route: usize, message: Message<'_>) {
        let frame = self.machines[route].binding().frame(message);
        let mut out = [0; 128];
        let n = encode_external_fore_session_frame_into(frame, &mut out, 16, 128).unwrap();
        self.machines[route].admit_outbound(frame).unwrap();
        self.queue.push_back(out[..n].to_vec());
    }
}
impl Frames for Peer {
    fn send(&mut self, bytes: &[u8], _: Duration) -> Result<(), ()> {
        self.writes += 1;
        if self.fail_send || self.fail_send_at == Some(self.writes) {
            return Err(());
        }
        let frame = decode_external_fore_session_frame(bytes, 16, 128).unwrap();
        let route = self
            .machines
            .iter()
            .position(|m| m.binding().scope_commitment == frame.scope_commitment)
            .unwrap();
        self.machines[route].admit_inbound(frame).unwrap();
        match frame.message {
            Message::Hello if route == 1 && self.delay_second_hello => {}
            Message::Hello => self.emit(route, Message::Hello),
            Message::Message(SessionMessage::Ready) => {
                self.emit(route, Message::Message(SessionMessage::Ready));
                if route == 0 && self.delay_second_hello {
                    self.emit(1, Message::Hello);
                }
            }
            Message::Message(SessionMessage::Offered { sequence, .. }) => {
                if self.pressure {
                    self.pressure = false;
                    self.emit(
                        route,
                        Message::Message(SessionMessage::Pressure { sequence }),
                    );
                } else {
                    self.emit(
                        route,
                        Message::Message(SessionMessage::Accepted { sequence }),
                    );
                    if self.early_reply {
                        if let Some(payload) = self.reply.take() {
                            self.emit(
                                1,
                                Message::Message(SessionMessage::Offered { sequence, payload }),
                            );
                        }
                    }
                    self.emit(
                        route,
                        Message::Message(SessionMessage::Delivered { sequence }),
                    );
                    if let Some(payload) = self.reply.take() {
                        self.emit(
                            1,
                            Message::Message(SessionMessage::Offered { sequence, payload }),
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn receive<'a>(&mut self, output: &'a mut [u8], _: Duration) -> Result<&'a [u8], ()> {
        if self.fail_receive {
            return Err(());
        }
        let bytes = self.queue.pop_front().ok_or(())?;
        if bytes.len() > output.len() {
            return Err(());
        }
        output[..bytes.len()].copy_from_slice(&bytes);
        Ok(&output[..bytes.len()])
    }
}
fn carriage() -> Carriage<Peer> {
    Carriage {
        frames: Peer::new(),
        state: State::prepare(bindings(), bounds()).unwrap(),
    }
}
fn active(c: &mut Carriage<Peer>) {
    for route in 0..2 {
        c.send(route, Message::Hello, TIMEOUT).unwrap();
        let mut out = [0; 128];
        assert_eq!(c.receive(&mut out, TIMEOUT).unwrap().route, route);
        c.send(route, Message::Message(SessionMessage::Ready), TIMEOUT)
            .unwrap();
        c.receive(&mut out, TIMEOUT).unwrap();
    }
    assert!(c.state.machines.iter().all(Machine::is_active));
}
#[test]
fn opaque_application_exchange_preserves_pressure_and_distinct_delivery_without_retry() {
    let mut c = carriage();
    active(&mut c);
    c.frames.pressure = true;
    let count = 7u64.to_le_bytes();
    let request = Message::Message(SessionMessage::Offered {
        sequence: 0,
        payload: &count,
    });
    c.send(0, request, TIMEOUT).unwrap();
    let writes = c.frames.writes;
    let mut out = [0; 128];
    assert!(matches!(
        c.receive(&mut out, TIMEOUT).unwrap().frame.message,
        Message::Message(SessionMessage::Pressure { sequence: 0 })
    ));
    assert_eq!(c.frames.writes, writes, "receiving pressure never retries");
    assert_eq!(c.state.machines[0].next_sequence(), 0);
    c.send(0, request, TIMEOUT).unwrap();
    c.receive(&mut out, TIMEOUT).unwrap();
    assert_eq!(c.state.machines[0].next_sequence(), 0);
    c.receive(&mut out, TIMEOUT).unwrap();
    assert_eq!(c.state.machines[0].next_sequence(), 1);
    c.frames.emit(
        1,
        Message::Message(SessionMessage::Offered {
            sequence: 0,
            payload: b"typed-result",
        }),
    );
    let value = c.receive(&mut out, TIMEOUT).unwrap();
    assert_eq!(value.route, 1);
    assert_eq!(
        value.frame.message,
        Message::Message(SessionMessage::Offered {
            sequence: 0,
            payload: b"typed-result"
        })
    );
    c.send(
        1,
        Message::Message(SessionMessage::Accepted { sequence: 0 }),
        TIMEOUT,
    )
    .unwrap();
    assert_eq!(c.frames.machines[1].next_sequence(), 0);
    c.send(
        1,
        Message::Message(SessionMessage::Delivered { sequence: 0 }),
        TIMEOUT,
    )
    .unwrap();
    assert_eq!(
        c.frames.machines[1].next_sequence(),
        1,
        "Wire delivery is not durable commit"
    );
}
#[test]
fn wrong_scope_malformed_truncated_and_reordered_frames_retire_without_application_delivery() {
    for case in 0..4 {
        let mut c = carriage();
        active(&mut c);
        let mut bytes = [0; 128];
        let mut frame =
            bindings()[1].frame(Message::Message(SessionMessage::Delivered { sequence: 0 }));
        if case == 0 {
            frame.scope_commitment = [9; 32];
        }
        let n = encode_external_fore_session_frame_into(frame, &mut bytes, 16, 128).unwrap();
        if case == 1 {
            bytes[0] ^= 1;
        }
        c.frames
            .queue
            .push_back(bytes[..if case == 2 { n - 1 } else { n }].to_vec());
        let mut out = [0; 128];
        assert!(c.receive(&mut out, TIMEOUT).is_err());
        assert!(c.state.retired);
        assert!(matches!(
            c.receive(&mut out, TIMEOUT),
            Err(ForeCarriageError::Retired)
        ));
    }
}
#[test]
fn configuration_frame_budget_timeout_partial_io_and_local_cancel_are_fail_closed() {
    let mut invalid = bounds();
    invalid.maximum_buffer_bytes = 127;
    assert!(matches!(
        State::prepare(bindings(), invalid),
        Err(ForeCarriageError::Bounds)
    ));
    let mut same = bindings();
    same[1].scope_commitment = same[0].scope_commitment;
    assert!(State::prepare(same, bounds()).is_err());
    let mut huge = bounds();
    huge.maximum_frame_bytes = 4097;
    huge.maximum_buffer_bytes = 4097;
    assert!(State::prepare(bindings(), huge).is_err());
    let mut c = carriage();
    active(&mut c);
    c.state.bounds.maximum_frames = c.state.used_frames;
    let before = c.frames.writes;
    assert_eq!(
        c.send(0, Message::Hello, TIMEOUT),
        Err(ForeCarriageError::Bounds)
    );
    assert_eq!(c.frames.writes, before);
    let mut c = carriage();
    assert_eq!(
        c.send(0, Message::Hello, Duration::ZERO),
        Err(ForeCarriageError::Bounds)
    );
    assert_eq!(c.frames.writes, 0);
    let mut c = carriage();
    c.frames.fail_send = true;
    assert_eq!(
        c.send(0, Message::Hello, TIMEOUT),
        Err(ForeCarriageError::OutcomeUnknown)
    );
    assert_eq!(
        c.send(0, Message::Hello, TIMEOUT),
        Err(ForeCarriageError::Retired)
    );
    assert_eq!(c.frames.writes, 1);
    let mut c = carriage();
    c.frames.fail_receive = true;
    let mut out = [0; 128];
    assert!(matches!(
        c.receive(&mut out, TIMEOUT),
        Err(ForeCarriageError::OutcomeUnknown)
    ));
    assert!(c.state.retired);
    let mut c = carriage();
    active(&mut c);
    let before = c.frames.writes;
    assert!(matches!(
        c.send(
            0,
            Message::Message(SessionMessage::Offered {
                sequence: 0,
                payload: &[0; 17]
            }),
            TIMEOUT
        ),
        Err(ForeCarriageError::Wire(WireError::OversizedPayload))
    ));
    assert_eq!(c.frames.writes, before);
    let mut c = carriage();
    c.cancel();
    assert_eq!(
        c.send(0, Message::Hello, TIMEOUT),
        Err(ForeCarriageError::Retired)
    );
    assert_eq!(c.frames.writes, 0);
}

#[cfg(unix)]
#[path = "pty_tests.rs"]
mod pty_tests;

#[path = "handshake_tests.rs"]
mod handshake;

#[path = "exchange_tests.rs"]
mod exchange;
