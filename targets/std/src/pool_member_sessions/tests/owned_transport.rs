//! Actual local socket transport of fixture-selected sessions. These tests
//! prove framing and session identity, not live provider scheduling.
use super::*;
use conduit_wire::{decode_session_frame, encode_session_frame_into, SessionMessage};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

struct Endpoint {
    socket: UnixStream,
    binding: SessionBinding,
    machine: SessionMachine,
    input: Vec<u8>,
    output: Vec<u8>,
}

impl Endpoint {
    fn prepare(socket: UnixStream, binding: SessionBinding, role: SessionRole) -> Self {
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let maximum = binding.attachment.limits.maximum_frame_bytes as usize;
        Self {
            socket,
            machine: SessionMachine::new(binding.clone(), role).unwrap(),
            binding,
            input: vec![0; maximum],
            output: vec![0; maximum],
        }
    }

    fn send(&mut self, message: SessionMessage<'_>) {
        let frame = self.binding.frame(message);
        let length = encode_session_frame_into(
            frame,
            &mut self.output,
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )
        .unwrap();
        self.machine.admit_outbound(frame).unwrap();
        self.socket
            .write_all(&(length as u32).to_be_bytes())
            .unwrap();
        self.socket.write_all(&self.output[..length]).unwrap();
    }

    fn receive(&mut self) -> Result<SessionMessage<'_>, String> {
        let mut prefix = [0; 4];
        self.socket
            .read_exact(&mut prefix)
            .map_err(|e| e.to_string())?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length == 0 || length > self.input.len() {
            return Err("frame extent exceeds prepared storage".into());
        }
        self.socket
            .read_exact(&mut self.input[..length])
            .map_err(|e| e.to_string())?;
        let frame = decode_session_frame(
            &self.input[..length],
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )
        .map_err(|e| format!("{e:?}"))?;
        self.machine
            .admit_inbound(frame)
            .map_err(|e| format!("{e:?}"))?;
        Ok(frame.message)
    }

    fn hello(&mut self) {
        let binding = self.binding.clone();
        self.send(binding.hello_frame().message);
    }
}

fn binding() -> SessionBinding {
    let (plan, selection, consumer) = exact_plan();
    let sessions =
        PoolMemberSessions::prepare(&plan, selection, &consumer, &HostId::from("host/worker"))
            .unwrap();
    let binding = sessions
        .iter()
        .find(|s| s.direction == PoolMemberSessionDirection::Input)
        .unwrap()
        .binding()
        .clone();
    binding
}

#[test]
fn selected_request_session_crosses_owned_socket_with_exact_handshake() {
    let binding = binding();
    let (left, right) = UnixStream::pair().unwrap();
    let mut source = Endpoint::prepare(left, binding.clone(), SessionRole::Source);
    let worker = std::thread::spawn(move || {
        let mut sink = Endpoint::prepare(right, binding, SessionRole::Sink);
        assert!(matches!(sink.receive().unwrap(), SessionMessage::Hello(_)));
        sink.hello();
        assert!(matches!(sink.receive().unwrap(), SessionMessage::Ready));
        sink.send(SessionMessage::Ready);
        assert!(matches!(
            sink.receive().unwrap(),
            SessionMessage::Offered {
                sequence: 0,
                payload: b"bounded request"
            }
        ));
        sink.send(SessionMessage::Accepted { sequence: 0 });
        sink.send(SessionMessage::Delivered { sequence: 0 });
    });
    source.hello();
    assert!(matches!(
        source.receive().unwrap(),
        SessionMessage::Hello(_)
    ));
    source.send(SessionMessage::Ready);
    assert!(matches!(source.receive().unwrap(), SessionMessage::Ready));
    source.send(SessionMessage::Offered {
        sequence: 0,
        payload: b"bounded request",
    });
    assert!(matches!(
        source.receive().unwrap(),
        SessionMessage::Accepted { sequence: 0 }
    ));
    assert!(matches!(
        source.receive().unwrap(),
        SessionMessage::Delivered { sequence: 0 }
    ));
    worker.join().unwrap();
}

#[test]
fn oversized_socket_frame_refuses_before_reading_body_or_growing_storage() {
    let binding = binding();
    let (mut source, sink) = UnixStream::pair().unwrap();
    let mut sink = Endpoint::prepare(sink, binding, SessionRole::Sink);
    let pointer = sink.input.as_ptr();
    let capacity = sink.input.capacity();
    source.write_all(&u32::MAX.to_be_bytes()).unwrap();
    assert_eq!(
        sink.receive().unwrap_err(),
        "frame extent exceeds prepared storage"
    );
    assert_eq!(sink.input.as_ptr(), pointer);
    assert_eq!(sink.input.capacity(), capacity);
    assert!(!sink.machine.is_active());
}

#[test]
fn selected_session_outbound_hello_uses_prepared_binding() {
    let (plan, selection, consumer) = exact_plan();
    let mut sessions =
        PoolMemberSessions::prepare(&plan, selection, &consumer, &HostId::from("host/worker"))
            .unwrap();
    let session = sessions.iter_mut().next().unwrap();
    let binding = session.binding().clone();
    let hello = binding.hello_frame().message;
    session.admit_outbound_message(hello).unwrap();
    assert!(session.admit_outbound_message(hello).is_err());
}
