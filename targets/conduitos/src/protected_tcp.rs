//! Root owns device/time/endpoint custody; the independent guest owns TCP state.
mod flow;
mod transport;
use crate::{
    arch::{CandidateDeadline, VirtioNetError, VirtioNetReady},
    virtio_tcp::VirtioTcpError,
    virtio_tcp_stream::{
        PROOF_POLLS_PER_PROTOCOL_TICK, VirtioTcpConnectOptions, VirtioTcpIoError,
        report_read_refusal,
    },
};
use conduitos_tcp_core::{
    FRAME_BYTES, SOCKET_BYTES,
    state::{INITIALIZE_BYTES, Initialize},
    wire::{Request, Response},
};
use embedded_io::{ErrorType, Read, Write};
use flow::Flow;
use transport::Service;
#[derive(Clone, Copy)]
struct Status {
    state: u32,
    receive: u32,
    send: u32,
    frames: u32,
    next_frame: u32,
    flags: u32,
}
pub(crate) struct Stream {
    device: VirtioNetReady,
    service: Service,
    flow: Flow,
    maximum_polls: u32,
    polls: u32,
    deadline: Option<CandidateDeadline>,
    last_sent_frame: u32,
    failure: Option<VirtioTcpError>,
}
impl Stream {
    pub(crate) fn connect(
        device: VirtioNetReady,
        options: VirtioTcpConnectOptions,
    ) -> Result<Self, VirtioTcpError> {
        if options.maximum_polls == 0 {
            return Err(VirtioTcpError::InvalidEndpoint);
        }
        let mac = device.identity().mac;
        let flow = Flow::new(mac, options.endpoint)?;
        let service = Service::install()?;
        let mut stream = Self {
            device,
            service,
            flow,
            maximum_polls: options.maximum_polls,
            polls: 0,
            deadline: options.deadline,
            last_sent_frame: 0,
            failure: None,
        };
        let endpoint = options.endpoint;
        let mut initialize = [0; INITIALIZE_BYTES];
        Initialize {
            seed: options.random_seed,
            mac,
            prefix: endpoint.prefix_length,
            local: endpoint.guest_address,
            gateway: endpoint.gateway,
            remote: endpoint.remote_address,
            local_port: endpoint.local_port,
            remote_port: endpoint.remote_port,
            millis: 0,
        }
        .encode(&mut initialize)
        .map_err(|_| VirtioTcpError::InvalidEndpoint)?;
        stream.checked(|s| {
            s.service.input(&initialize)?;
            if s.service.request(Request::Init)? != Response::Ready {
                return Err(VirtioTcpError::ConnectRefused);
            }
            for _ in 0..s.maximum_polls {
                let status = s.poll()?;
                if status.state == 4 {
                    return Ok(());
                }
                if status.flags & 8 == 0 {
                    return Err(VirtioTcpError::ConnectRefused);
                }
            }
            Err(VirtioTcpError::Timeout)
        })?;
        Ok(stream)
    }
    fn checked<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, VirtioTcpError>,
    ) -> Result<T, VirtioTcpError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = operation(self);
        if let Err(error) = result.as_ref() {
            self.failure = Some(*error);
            self.service.abort();
        }
        result
    }
    fn status(&mut self, response: Response) -> Result<Status, VirtioTcpError> {
        let Response::Status {
            state,
            receive,
            send,
            frames,
            next_frame,
            polls,
            flags,
        } = response
        else {
            return Err(VirtioTcpError::ReceiveRefused);
        };
        if polls != self.polls {
            return Err(VirtioTcpError::ReceiveRefused);
        }
        Ok(Status {
            state,
            receive,
            send,
            frames,
            next_frame,
            flags,
        })
    }
    fn current(&mut self) -> Result<Status, VirtioTcpError> {
        let response = self.service.request(Request::Status)?;
        self.status(response)
    }
    fn drain(&mut self, mut status: Status) -> Result<Status, VirtioTcpError> {
        let mut frame = [0; FRAME_BYTES];
        for _ in 0..4 {
            if status.frames == 0 {
                return Ok(status);
            }
            let id = status.next_frame;
            if id <= self.last_sent_frame {
                return Err(VirtioTcpError::SendRefused);
            }
            let Response::Frame {
                frame: returned,
                bytes,
            } = self.service.request(Request::TakeFrame { frame: id })?
            else {
                return Err(VirtioTcpError::SendRefused);
            };
            if returned != id || bytes == 0 || bytes as usize > frame.len() {
                return Err(VirtioTcpError::SendRefused);
            }
            let bytes = bytes as usize;
            self.service.output(&mut frame[..bytes])?;
            if !self.flow.egress(&frame[..bytes]) {
                return Err(VirtioTcpError::SendRefused);
            }
            // A published frame is never replayed after device or Ack failure.
            // The high-water mark is an effect ledger, not guest TCP state.
            self.last_sent_frame = id;
            self.device
                .send(&frame[..bytes], 1_000_000)
                .map_err(VirtioTcpError::Device)?;
            if self.service.request(Request::AckFrame { frame: id })? != Response::Ready {
                return Err(VirtioTcpError::SendRefused);
            }
            let next = self.current()?;
            if next.frames + 1 != status.frames {
                return Err(VirtioTcpError::SendRefused);
            }
            status = next;
        }
        if status.frames != 0 {
            return Err(VirtioTcpError::SendRefused);
        }
        Ok(status)
    }
    fn poll(&mut self) -> Result<Status, VirtioTcpError> {
        let current = self.current()?;
        self.drain(current)?;
        self.polls = self.polls.checked_add(1).ok_or(VirtioTcpError::Timeout)?;
        let millis = match self.deadline {
            Some(deadline) => {
                u64::try_from(deadline.elapsed_millis().ok_or(VirtioTcpError::Timeout)?)
                    .map_err(|_| VirtioTcpError::ClockRegressed)?
            }
            None => u64::from(self.polls / PROOF_POLLS_PER_PROTOCOL_TICK),
        };
        let mut frame = [0; FRAME_BYTES];
        let ingress = match self.device.receive(&mut frame) {
            Ok(bytes) if bytes <= frame.len() => {
                if self.flow.ingress(&frame[..bytes]) {
                    self.service.input(&frame[..bytes])?;
                    bytes as u32
                } else {
                    0
                }
            }
            Ok(_) => return Err(VirtioTcpError::ReceiveRefused),
            Err(VirtioNetError::Pressure) => 0,
            Err(error) => return Err(VirtioTcpError::Device(error)),
        };
        let response = self.service.request(Request::Poll { millis, ingress })?;
        let status = self.status(response)?;
        self.drain(status)
    }
    pub(crate) fn close_tcp_with_device(mut self) -> Result<(VirtioNetReady, u32), VirtioTcpError> {
        self.checked(|s| {
            if s.service.request(Request::Close)? != Response::Ready {
                return Err(VirtioTcpError::SendRefused);
            }
            for _ in 0..s.maximum_polls {
                let status = s.poll()?;
                if matches!(status.state, 0 | 6 | 10) {
                    if s.service.request(Request::Abort)? != Response::Ready {
                        return Err(VirtioTcpError::SendRefused);
                    }
                    s.service.abort();
                    return Ok(());
                }
            }
            Err(VirtioTcpError::Timeout)
        })?;
        Ok((self.device, self.polls))
    }
    fn read_inner(&mut self, output: &mut [u8]) -> Result<usize, VirtioTcpError> {
        let limit = if self.deadline.is_some() {
            u32::MAX
        } else {
            self.maximum_polls
        };
        for _ in 0..limit {
            let status = self
                .poll()
                .inspect_err(|error| report_read_refusal(error.as_str()))?;
            if status.receive > 0 {
                let maximum = output.len().min(SOCKET_BYTES) as u32;
                let Response::Read { bytes } = self.service.request(Request::Read { maximum })?
                else {
                    return Err(VirtioTcpError::ReceiveRefused);
                };
                if bytes == 0 || bytes > maximum || bytes > status.receive {
                    return Err(VirtioTcpError::ReceiveRefused);
                }
                self.service.output(&mut output[..bytes as usize])?;
                return Ok(bytes as usize);
            }
            if status.flags & 4 == 0 || status.flags & 8 == 0 {
                report_read_refusal("tcp-peer-closed");
                return Ok(0);
            }
        }
        report_read_refusal("tcp-read-poll-bound");
        Err(VirtioTcpError::Timeout)
    }
    fn write_inner(&mut self, input: &[u8]) -> Result<usize, VirtioTcpError> {
        for _ in 0..self.maximum_polls {
            let status = self.poll()?;
            if status.flags & 1 != 0 {
                let count = input.len().min(SOCKET_BYTES);
                self.service.input(&input[..count])?;
                let Response::Written { bytes } = self.service.request(Request::Write {
                    bytes: count as u32,
                })?
                else {
                    return Err(VirtioTcpError::SendRefused);
                };
                if bytes == 0 || bytes as usize > count {
                    return Err(VirtioTcpError::SendRefused);
                }
                return Ok(bytes as usize);
            }
            if status.flags & 2 == 0 || status.flags & 8 == 0 {
                return Err(VirtioTcpError::RemoteClosed);
            }
        }
        Err(VirtioTcpError::Timeout)
    }
    fn flush_inner(&mut self) -> Result<(), VirtioTcpError> {
        for _ in 0..self.maximum_polls {
            let status = self.poll()?;
            if status.send == 0 {
                return Ok(());
            }
            if status.flags & 8 == 0 {
                return Err(VirtioTcpError::RemoteClosed);
            }
        }
        Err(VirtioTcpError::Timeout)
    }
}
impl ErrorType for Stream {
    type Error = VirtioTcpIoError;
}
impl Read for Stream {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
        if output.is_empty() {
            return Ok(0);
        }
        self.checked(|s| s.read_inner(output))
            .map_err(VirtioTcpIoError)
    }
}
impl Write for Stream {
    fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
        if input.is_empty() {
            return Ok(0);
        }
        self.checked(|s| s.write_inner(input))
            .map_err(VirtioTcpIoError)
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.checked(Self::flush_inner).map_err(VirtioTcpIoError)
    }
}
