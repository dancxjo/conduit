//! Actual Carriage::handshake with existing codecs/session machines and an
//! explicitly bounded simulated peer FIFO. Software scheduling proof, not UART.
use super::*;
struct ScheduledPeer {
    machines: [Machine; 2],
    to_client: std::collections::VecDeque<Vec<u8>>,
    to_peer: std::collections::VecDeque<Vec<u8>>,
    unread: usize,
    peak: usize,
    schedule: u16,
    sends: usize,
    reads: usize,
    first_route: usize,
    fail_at: Option<usize>,
}
impl ScheduledPeer {
    fn new(schedule: u16, first_route: usize) -> Self {
        let [a, b] = bindings();
        let mut peer = Self {
            machines: [
                Machine::new(a, SessionRole::Sink).unwrap(),
                Machine::new(b, SessionRole::Source).unwrap(),
            ],
            to_client: Default::default(),
            to_peer: Default::default(),
            unread: 0,
            peak: 0,
            schedule,
            sends: 0,
            reads: 0,
            first_route,
            fail_at: None,
        };
        peer.emit(first_route, Message::Hello);
        peer.emit(1 - first_route, Message::Hello);
        peer
    }
    fn emit(&mut self, route: usize, message: Message<'_>) {
        let binding = self.machines[route].binding();
        let frame = binding.frame(message);
        self.machines[route].admit_outbound(frame).unwrap();
        let mut buffer = [0; 128];
        let n = encode_external_fore_session_frame_into(frame, &mut buffer, 16, 128).unwrap();
        self.to_client.push_back(buffer[..n].to_vec());
    }
    fn advance(&mut self) -> bool {
        let Some(bytes) = self.to_peer.pop_front() else {
            return false;
        };
        self.unread -= bytes.len() + 2;
        let frame = decode_external_fore_session_frame(&bytes, 16, 128).unwrap();
        let route = self
            .machines
            .iter()
            .position(|m| m.binding().scope_commitment == frame.scope_commitment)
            .unwrap();
        self.machines[route].admit_inbound(frame).unwrap();
        if frame.message == Message::Hello {
            self.emit(route, Message::Message(SessionMessage::Ready));
        }
        true
    }
}
impl Frames for ScheduledPeer {
    fn send(&mut self, bytes: &[u8], _: Duration) -> Result<(), ()> {
        assert!(
            self.reads > 0,
            "production client must listen before its first write"
        );
        self.sends += 1;
        if self.fail_at == Some(self.sends) {
            return Err(());
        }
        let frame = decode_external_fore_session_frame(bytes, 16, 128).unwrap();
        let route = if frame.scope_commitment == bindings()[0].scope_commitment {
            0
        } else {
            1
        };
        if self.sends <= 2 {
            assert_eq!(
                route, self.first_route,
                "other Hello must be retained without responding early"
            );
        }
        self.unread += bytes.len() + 2;
        self.peak = self.peak.max(self.unread);
        assert!(
            self.unread <= 128,
            "actual client overflowed the declared peer FIFO"
        );
        self.to_peer.push_back(bytes.to_vec());
        let turns = (self.schedule >> ((self.sends - 1) * 2)) & 3;
        for _ in 0..turns {
            self.advance();
        }
        Ok(())
    }
    fn receive<'a>(&mut self, output: &'a mut [u8], _: Duration) -> Result<&'a [u8], ()> {
        self.reads += 1;
        while self.to_client.is_empty() {
            if !self.advance() {
                return Err(());
            }
        }
        let bytes = self.to_client.pop_front().unwrap();
        if bytes.len() > output.len() {
            return Err(());
        }
        output[..bytes.len()].copy_from_slice(&bytes);
        Ok(&output[..bytes.len()])
    }
}
#[test]
fn actual_client_keeps_both_peer_hellos_and_serializes_activation_under_all_schedules() {
    let mut observed_peak = 0;
    for first_route in 0..2 {
        for schedule in 0..256 {
            let mut client = Carriage {
                frames: ScheduledPeer::new(schedule, first_route),
                state: State::prepare(bindings(), bounds()).unwrap(),
            };
            client.handshake(&mut [0; 128], TIMEOUT).unwrap();
            assert!(client.state.machines.iter().all(Machine::is_active));
            assert_eq!(client.state.used_frames, 8);
            assert_eq!((client.frames.reads, client.frames.sends), (4, 4));
            observed_peak = observed_peak.max(client.frames.peak);
            while client.frames.advance() {}
            assert!(client.frames.machines.iter().all(Machine::is_active));
        }
    }
    assert_eq!(
        observed_peak, 126,
        "42-byte prior Ready plus next84-byte pair"
    );
}
#[test]
fn interruption_in_second_route_pair_retires_actual_client_without_replay() {
    let mut peer = ScheduledPeer::new(0, 0);
    peer.fail_at = Some(4);
    let mut client = Carriage {
        frames: peer,
        state: State::prepare(bindings(), bounds()).unwrap(),
    };
    assert_eq!(
        client.handshake(&mut [0; 128], TIMEOUT),
        Err(ForeCarriageError::OutcomeUnknown)
    );
    assert!(client.state.retired);
    let frames = client.state.used_frames;
    assert_eq!(
        client.handshake(&mut [0; 128], TIMEOUT),
        Err(ForeCarriageError::Retired)
    );
    assert_eq!(client.state.used_frames, frames);
    assert_eq!(client.frames.sends, 4);
}
