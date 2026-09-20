//! Exact #1211 session admission over the native WebRTC transport.

use conduit_wire::{
    SessionBinding, SessionMachine, SessionMessage, SessionRole, SessionTerminalDisposition,
    WireError, decode_session_frame, encode_session_frame_into,
};

use super::{NativeWebRtcEndpoint, NativeWebRtcRefusal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWebRtcSessionRefusal {
    Transport(NativeWebRtcRefusal),
    Wire(WireError),
    WrongBase,
    WrongStage,
    OutputBound,
}

impl From<NativeWebRtcRefusal> for NativeWebRtcSessionRefusal {
    fn from(value: NativeWebRtcRefusal) -> Self {
        Self::Transport(value)
    }
}

impl From<WireError> for NativeWebRtcSessionRefusal {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}

pub struct NativeWebRtcSession {
    endpoint: NativeWebRtcEndpoint,
    binding: SessionBinding,
    machine: SessionMachine,
    role: SessionRole,
    input: Box<[u8]>,
    output: Box<[u8]>,
    active: bool,
}

impl NativeWebRtcSession {
    pub fn new(
        endpoint: NativeWebRtcEndpoint,
        binding: SessionBinding,
        role: SessionRole,
    ) -> Result<Self, NativeWebRtcSessionRefusal> {
        binding.validate()?;
        if binding.attachment.base.as_str() != "conduit.base/webrtc-data-channel@1" {
            return Err(NativeWebRtcSessionRefusal::WrongBase);
        }
        let capacity = binding.attachment.limits.maximum_frame_bytes as usize;
        if capacity == 0 || capacity > endpoint.maximum_frame_bytes() {
            return Err(NativeWebRtcSessionRefusal::OutputBound);
        }
        let machine = SessionMachine::new(binding.clone(), role)?;
        Ok(Self {
            endpoint,
            binding,
            machine,
            role,
            input: vec![0; capacity].into_boxed_slice(),
            output: vec![0; capacity].into_boxed_slice(),
            active: false,
        })
    }

    pub async fn handshake(&mut self) -> Result<(), NativeWebRtcSessionRefusal> {
        if self.active {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        }
        let binding = self.binding.clone();
        let hello = binding.hello_frame();
        self.machine.admit_outbound(hello)?;
        self.send(hello).await?;
        let peer_hello_length = self.receive_into_input().await?;
        let peer_hello = decode_session_frame(
            &self.input[..peer_hello_length],
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )?;
        if !matches!(peer_hello.message, SessionMessage::Hello(_)) {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        }
        self.machine.admit_inbound(peer_hello)?;

        let ready = binding.frame(SessionMessage::Ready);
        self.machine.admit_outbound(ready)?;
        self.send(ready).await?;
        let peer_ready_length = self.receive_into_input().await?;
        let peer_ready = decode_session_frame(
            &self.input[..peer_ready_length],
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )?;
        if !matches!(peer_ready.message, SessionMessage::Ready) {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        }
        self.machine.admit_inbound(peer_ready)?;
        self.active = true;
        Ok(())
    }

    pub async fn offer_and_wait_delivery(
        &mut self,
        payload: &[u8],
    ) -> Result<u64, NativeWebRtcSessionRefusal> {
        if !self.active || self.role != SessionRole::Source {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        }
        let sequence = self.machine.next_sequence();
        let binding = self.binding.clone();
        let offered = binding.frame(SessionMessage::Offered { sequence, payload });
        self.machine.admit_outbound(offered)?;
        self.send(offered).await?;
        for expected_delivery in [false, true] {
            let response_length = self.receive_into_input().await?;
            let response = decode_session_frame(
                &self.input[..response_length],
                self.binding.limits.maximum_payload_bytes,
                self.binding.attachment.limits.maximum_frame_bytes,
            )?;
            let exact = if expected_delivery {
                matches!(response.message, SessionMessage::Delivered { sequence: actual } if actual == sequence)
            } else {
                matches!(response.message, SessionMessage::Accepted { sequence: actual } if actual == sequence)
            };
            if !exact {
                return Err(NativeWebRtcSessionRefusal::WrongStage);
            }
            self.machine.admit_inbound(response)?;
        }
        Ok(sequence)
    }

    pub async fn receive_and_deliver(
        &mut self,
        value: &mut [u8],
    ) -> Result<(u64, usize), NativeWebRtcSessionRefusal> {
        if !self.active || self.role != SessionRole::Sink {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        }
        let offered_length = self.receive_into_input().await?;
        let offered = decode_session_frame(
            &self.input[..offered_length],
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )?;
        let SessionMessage::Offered { sequence, payload } = offered.message else {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        };
        if payload.len() > value.len() {
            return Err(NativeWebRtcSessionRefusal::OutputBound);
        }
        value[..payload.len()].copy_from_slice(payload);
        let length = payload.len();
        self.machine.admit_inbound(offered)?;
        let binding = self.binding.clone();
        for message in [
            SessionMessage::Accepted { sequence },
            SessionMessage::Delivered { sequence },
        ] {
            let frame = binding.frame(message);
            self.machine.admit_outbound(frame)?;
            self.send(frame).await?;
        }
        Ok((sequence, length))
    }

    pub async fn finish(mut self) -> Result<(), NativeWebRtcSessionRefusal> {
        if !self.active {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        }
        let binding = self.binding.clone();
        if self.role == SessionRole::Source {
            let input_closed = binding.frame(SessionMessage::InputClosed {
                final_sequence: self.machine.next_sequence(),
            });
            self.machine.admit_outbound(input_closed)?;
            self.send(input_closed).await?;
        } else {
            let input_closed_length = self.receive_into_input().await?;
            let input_closed = decode_session_frame(
                &self.input[..input_closed_length],
                self.binding.limits.maximum_payload_bytes,
                self.binding.attachment.limits.maximum_frame_bytes,
            )?;
            if !matches!(input_closed.message, SessionMessage::InputClosed { .. }) {
                return Err(NativeWebRtcSessionRefusal::WrongStage);
            }
            self.machine.admit_inbound(input_closed)?;
        }
        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: self.machine.next_sequence(),
        });
        self.machine.admit_outbound(terminal)?;
        self.send(terminal).await?;
        let peer_length = self.receive_into_input().await?;
        let peer = decode_session_frame(
            &self.input[..peer_length],
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )?;
        if !matches!(peer.message, SessionMessage::Terminal { .. }) {
            return Err(NativeWebRtcSessionRefusal::WrongStage);
        }
        self.machine.admit_inbound(peer)?;
        self.active = false;
        // The exact peer terminal exchange owns session completion. Dropping
        // the endpoint at return fences the transport without inventing a
        // second semantic acknowledgement from PeerConnection teardown.
        Ok(())
    }

    async fn send(
        &mut self,
        frame: conduit_wire::SessionFrame<'_>,
    ) -> Result<(), NativeWebRtcSessionRefusal> {
        let length = encode_session_frame_into(
            frame,
            &mut self.output,
            self.binding.limits.maximum_payload_bytes,
            self.binding.attachment.limits.maximum_frame_bytes,
        )?;
        self.endpoint.send(&self.output[..length]).await?;
        Ok(())
    }

    async fn receive_into_input(&mut self) -> Result<usize, NativeWebRtcSessionRefusal> {
        Ok(self.endpoint.receive(&mut self.input).await?)
    }
}
