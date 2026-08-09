//! Transport-neutral host execution for the two exact R1 Signal Plans.

use std::time::Duration;

use conduit_signal::SIGNAL_ENCODED_LEN;
use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_std_host::usb_cdc::NativePathCdcCarrier;
use conduit_std_host::websocket::NativeWebSocketCarrier;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionFrame, SessionMessage,
    SessionTerminalDisposition,
};

use super::PicoResult;

const SESSION_PAYLOAD_BYTES: u32 = SIGNAL_ENCODED_LEN;
const SESSION_FRAME_BYTES: u32 = conduit_net::R1_MAXIMUM_FRAME_BYTES;

pub trait R1SessionIo {
    fn send(&mut self, frame: &SessionFrame<'_>) -> PicoResult<()>;
    fn receive<'a>(&mut self, bytes: &'a mut [u8]) -> PicoResult<SessionFrame<'a>>;
}

pub struct WebSocketSessionIo<'a> {
    carrier: &'a mut NativeWebSocketCarrier,
}

impl<'a> WebSocketSessionIo<'a> {
    pub fn new(carrier: &'a mut NativeWebSocketCarrier) -> Self {
        Self { carrier }
    }
}

impl R1SessionIo for WebSocketSessionIo<'_> {
    fn send(&mut self, frame: &SessionFrame<'_>) -> PicoResult<()> {
        let mut bytes = [0_u8; SESSION_FRAME_BYTES as usize];
        let length = encode_session_frame_into(
            *frame,
            &mut bytes,
            SESSION_PAYLOAD_BYTES,
            SESSION_FRAME_BYTES,
        )
        .map_err(|error| format!("failed encoding R1 Session frame: {error:?}"))?;
        self.carrier
            .send_binary(&bytes[..length])
            .map_err(|error| format!("WebSocket send failed: {error:?}").into())
    }

    fn receive<'a>(&mut self, bytes: &'a mut [u8]) -> PicoResult<SessionFrame<'a>> {
        let length = self
            .carrier
            .receive_binary(bytes)
            .map_err(|error| format!("WebSocket receive failed: {error:?}"))?;
        decode_session_frame(&bytes[..length], SESSION_PAYLOAD_BYTES, SESSION_FRAME_BYTES)
            .map_err(|error| format!("failed decoding R1 Session frame: {error:?}").into())
    }
}

pub struct UsbSessionIo<'a> {
    carrier: &'a mut NativePathCdcCarrier,
}

impl<'a> UsbSessionIo<'a> {
    pub fn new(carrier: &'a mut NativePathCdcCarrier) -> Self {
        Self { carrier }
    }
}

impl R1SessionIo for UsbSessionIo<'_> {
    fn send(&mut self, frame: &SessionFrame<'_>) -> PicoResult<()> {
        self.carrier
            .send_frame(frame, Duration::from_secs(2))
            .map_err(Into::into)
    }

    fn receive<'a>(&mut self, bytes: &'a mut [u8]) -> PicoResult<SessionFrame<'a>> {
        self.carrier
            .receive_frame(bytes, Duration::from_secs(15))
            .map_err(Into::into)
    }
}

pub fn handshake(io: &mut impl R1SessionIo, source: &mut PicoUsbSource) -> PicoResult<()> {
    let binding = source.binding().clone();
    let hello = binding.hello_frame();
    source.admit_outbound(hello)?;
    io.send(&hello)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES as usize];
    let received = io.receive(&mut bytes)?;
    if !matches!(received.message, SessionMessage::Hello(_)) {
        return Err("R1 sink did not return exact Hello".into());
    }
    source.admit_inbound(received)?;

    let ready = binding.frame(SessionMessage::Ready);
    source.admit_outbound(ready)?;
    io.send(&ready)?;
    let received = io.receive(&mut bytes)?;
    if !matches!(received.message, SessionMessage::Ready) {
        return Err("R1 sink did not return exact Ready".into());
    }
    source.admit_inbound(received)?;
    if !source.is_active() {
        return Err("R1 session did not become active".into());
    }
    Ok(())
}

pub fn deliver_next(
    io: &mut impl R1SessionIo,
    source: &mut PicoUsbSource,
    after_accepted: &mut impl FnMut(u64) -> PicoResult<()>,
) -> PicoResult<bool> {
    let Some((sequence, payload)) = source.next_offer()? else {
        return Ok(false);
    };
    let binding = source.binding().clone();
    loop {
        let offered = binding.frame(SessionMessage::Offered {
            sequence,
            payload: &payload,
        });
        source.admit_outbound(offered)?;
        io.send(&offered)?;
        let mut bytes = [0_u8; SESSION_FRAME_BYTES as usize];
        let response = io.receive(&mut bytes)?;
        source.admit_inbound(response)?;
        match response.message {
            SessionMessage::Pressure { sequence: found } if found == sequence => {
                source.pressure(sequence)?;
            }
            SessionMessage::Accepted { sequence: found } if found == sequence => {
                source.accepted(sequence)?;
                break;
            }
            _ => return Err("R1 sink returned an unexpected offer disposition".into()),
        }
    }
    after_accepted(sequence)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES as usize];
    let delivered = io.receive(&mut bytes)?;
    source.admit_inbound(delivered)?;
    if !matches!(delivered.message, SessionMessage::Delivered { sequence: found } if found == sequence)
    {
        return Err("R1 sink returned an unexpected delivery disposition".into());
    }
    source.delivered(sequence)?;
    Ok(true)
}

pub fn replay_offered(
    io: &mut impl R1SessionIo,
    source: &mut PicoUsbSource,
    sequence: u64,
    payload: &[u8; SIGNAL_ENCODED_LEN as usize],
    after_accepted: &mut impl FnMut(u64) -> PicoResult<()>,
) -> PicoResult<()> {
    let binding = source.binding().clone();
    let offered = binding.frame(SessionMessage::Offered { sequence, payload });
    // Reconciliation retained this exact Offered transfer in the source
    // machine, so retransmission must not admit it as a new offer.
    io.send(&offered)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES as usize];
    let accepted = io.receive(&mut bytes)?;
    source.admit_inbound(accepted)?;
    if !matches!(accepted.message, SessionMessage::Accepted { sequence: found } if found == sequence)
    {
        return Err("R1 sink did not accept the reconciled replay".into());
    }
    source.accepted(sequence)?;
    after_accepted(sequence)?;
    let delivered = io.receive(&mut bytes)?;
    source.admit_inbound(delivered)?;
    if !matches!(delivered.message, SessionMessage::Delivered { sequence: found } if found == sequence)
    {
        return Err("R1 sink did not deliver the reconciled replay".into());
    }
    source.delivered(sequence)?;
    Ok(())
}

pub fn finish(io: &mut impl R1SessionIo, source: &mut PicoUsbSource) -> PicoResult<u64> {
    let final_sequence = source.finish_kernel()?;
    let binding = source.binding().clone();
    let closed = binding.frame(SessionMessage::InputClosed { final_sequence });
    source.admit_outbound(closed)?;
    io.send(&closed)?;
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence,
    });
    source.admit_outbound(terminal)?;
    io.send(&terminal)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES as usize];
    let response = io.receive(&mut bytes)?;
    if !matches!(
        response.message,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: found,
        } if found == final_sequence
    ) {
        return Err("R1 sink returned an unexpected terminal disposition".into());
    }
    source.admit_inbound(response)?;
    if !source.is_terminal() {
        return Err("R1 session did not reach reciprocal terminal agreement".into());
    }
    Ok(final_sequence)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use conduit_core::{BootId, HostId};
    use conduit_wire::{SessionBinding, SessionMachine, SessionRole};

    use super::*;

    #[derive(Clone, Copy)]
    enum Response {
        Hello,
        Ready,
        Pressure(u64),
        Accepted(u64),
        Delivered(u64),
        Terminal(u64),
    }

    struct FakeIo {
        binding: SessionBinding,
        sink: SessionMachine,
        responses: VecDeque<Response>,
        pressured_once: bool,
    }

    impl FakeIo {
        fn new(binding: SessionBinding) -> Self {
            Self {
                sink: SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap(),
                binding,
                responses: VecDeque::new(),
                pressured_once: false,
            }
        }
    }

    impl R1SessionIo for FakeIo {
        fn send(&mut self, frame: &SessionFrame<'_>) -> PicoResult<()> {
            self.sink
                .admit_inbound(*frame)
                .map_err(|error| format!("fake sink rejected inbound: {error:?}"))?;
            match frame.message {
                SessionMessage::Hello(_) => self.responses.push_back(Response::Hello),
                SessionMessage::Ready => self.responses.push_back(Response::Ready),
                SessionMessage::Offered { sequence, .. } if !self.pressured_once => {
                    self.pressured_once = true;
                    self.responses.push_back(Response::Pressure(sequence));
                }
                SessionMessage::Offered { sequence, .. } => {
                    self.responses.push_back(Response::Accepted(sequence));
                    self.responses.push_back(Response::Delivered(sequence));
                }
                SessionMessage::InputClosed { .. } => {}
                SessionMessage::Terminal { final_sequence, .. } => {
                    self.responses.push_back(Response::Terminal(final_sequence));
                }
                _ => return Err("fake sink received an unexpected source frame".into()),
            }
            Ok(())
        }

        fn receive<'a>(&mut self, bytes: &'a mut [u8]) -> PicoResult<SessionFrame<'a>> {
            let response = self
                .responses
                .pop_front()
                .ok_or("fake sink has no queued response")?;
            let binding = self.binding.clone();
            let frame = match response {
                Response::Hello => binding.hello_frame(),
                Response::Ready => binding.frame(SessionMessage::Ready),
                Response::Pressure(sequence) => {
                    binding.frame(SessionMessage::Pressure { sequence })
                }
                Response::Accepted(sequence) => {
                    binding.frame(SessionMessage::Accepted { sequence })
                }
                Response::Delivered(sequence) => {
                    binding.frame(SessionMessage::Delivered { sequence })
                }
                Response::Terminal(final_sequence) => binding.frame(SessionMessage::Terminal {
                    disposition: SessionTerminalDisposition::Completed,
                    final_sequence,
                }),
            };
            self.sink
                .admit_outbound(frame)
                .map_err(|error| format!("fake sink rejected outbound: {error:?}"))?;
            let length =
                encode_session_frame_into(frame, bytes, SESSION_PAYLOAD_BYTES, SESSION_FRAME_BYTES)
                    .map_err(|error| format!("fake sink encode failed: {error:?}"))?;
            decode_session_frame(&bytes[..length], SESSION_PAYLOAD_BYTES, SESSION_FRAME_BYTES)
                .map_err(|error| format!("fake sink decode failed: {error:?}").into())
        }
    }

    #[test]
    fn transport_neutral_driver_executes_exact_r1_plan_with_pressure() {
        let exact = conduit_system_continuity::exact_r1_signal_plan(
            BootId::from(conduit_net::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )
        .unwrap();
        let mut source =
            PicoUsbSource::prepare_plan(exact.plan, &HostId::from(conduit_net::R1_STD_HOST_ID))
                .unwrap();
        let mut io = FakeIo::new(source.binding().clone());
        let mut accepted = Vec::new();
        handshake(&mut io, &mut source).unwrap();
        while deliver_next(&mut io, &mut source, &mut |sequence| {
            accepted.push(sequence);
            Ok(())
        })
        .unwrap()
        {}
        let delivered = finish(&mut io, &mut source).unwrap();
        assert_eq!(delivered, 16);
        assert_eq!(accepted, (0..16).collect::<Vec<_>>());
        assert_eq!(source.pressure_retries(), 1);
    }
}
