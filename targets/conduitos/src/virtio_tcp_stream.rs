//! Blocking fixed-storage TCP stream used beneath reviewed protocol libraries.

use embedded_io::{Error, ErrorKind, ErrorType, Read, Write};
use smoltcp::{
    iface::{Interface, SocketHandle, SocketSet, SocketStorage},
    socket::tcp,
    time::Instant,
};

use crate::{
    arch::{CandidateDeadline, VirtioNetError, VirtioNetReady},
    virtio_tcp::{VirtioDevice, VirtioTcpEndpoint, VirtioTcpError},
};

/// Proof sessions without an admitted clock advance smoltcp's internal timer
/// conservatively. Product admission uses a measured candidate deadline.
const PROOF_POLLS_PER_PROTOCOL_TICK: u32 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VirtioTcpIoError(pub VirtioTcpError);

impl core::fmt::Display for VirtioTcpIoError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl core::error::Error for VirtioTcpIoError {}

impl Error for VirtioTcpIoError {
    fn kind(&self) -> ErrorKind {
        match self.0 {
            VirtioTcpError::InvalidEndpoint | VirtioTcpError::ClockRegressed => {
                ErrorKind::InvalidInput
            }
            VirtioTcpError::ConnectRefused => ErrorKind::ConnectionRefused,
            VirtioTcpError::RemoteClosed | VirtioTcpError::Cancelled => {
                ErrorKind::ConnectionAborted
            }
            VirtioTcpError::Timeout => ErrorKind::TimedOut,
            VirtioTcpError::SendRefused => ErrorKind::WriteZero,
            VirtioTcpError::ReceiveRefused
            | VirtioTcpError::RequestTooLarge
            | VirtioTcpError::ResponseTooLarge
            | VirtioTcpError::Device(_) => ErrorKind::Other,
        }
    }
}

pub(crate) struct VirtioTcpStream<'a> {
    adapter: VirtioDevice,
    interface: Interface,
    sockets: SocketSet<'a>,
    handle: SocketHandle,
    maximum_polls: u32,
    polls: u32,
    deadline: Option<CandidateDeadline>,
}

/// The admitted route and bounded polling basis for one TCP connection.
#[derive(Clone, Copy)]
pub(crate) struct VirtioTcpConnectOptions {
    pub random_seed: u64,
    pub endpoint: VirtioTcpEndpoint,
    pub maximum_polls: u32,
    pub deadline: Option<CandidateDeadline>,
}

impl<'a> VirtioTcpStream<'a> {
    pub(crate) fn connect(
        device: VirtioNetReady,
        options: VirtioTcpConnectOptions,
        receive_storage: &'a mut [u8],
        transmit_storage: &'a mut [u8],
        socket_storage: &'a mut [SocketStorage<'a>],
    ) -> Result<Self, VirtioTcpError> {
        let VirtioTcpConnectOptions {
            random_seed,
            endpoint,
            maximum_polls,
            deadline,
        } = options;
        if maximum_polls == 0
            || receive_storage.is_empty()
            || transmit_storage.is_empty()
            || socket_storage.len() != 1
            || endpoint.prefix_length == 0
            || endpoint.prefix_length > 32
            || endpoint.remote_port == 0
            || endpoint.local_port == 0
        {
            return Err(VirtioTcpError::InvalidEndpoint);
        }
        let mac = device.identity().mac;
        let mut adapter = VirtioDevice::new(device);
        let (interface, sockets, handle) = crate::virtio_tcp::session::prepare_session(
            &mut adapter,
            random_seed,
            mac,
            endpoint,
            Instant::ZERO,
            receive_storage,
            transmit_storage,
            socket_storage,
        )?;
        let mut stream = Self {
            adapter,
            interface,
            sockets,
            handle,
            maximum_polls,
            polls: 0,
            deadline,
        };
        for _ in 0..maximum_polls {
            stream.poll()?;
            let socket = stream.sockets.get::<tcp::Socket>(stream.handle);
            if socket.state() == tcp::State::Established {
                return Ok(stream);
            }
            if !socket.is_active() {
                return Err(VirtioTcpError::ConnectRefused);
            }
        }
        Err(VirtioTcpError::Timeout)
    }

    /// Return the same admitted VirtIO-net device after the TCP session
    /// retires. The hardware is initialized only once per Boot.
    pub(crate) fn close_tcp_with_device(mut self) -> Result<(VirtioNetReady, u32), VirtioTcpError> {
        self.sockets.get_mut::<tcp::Socket>(self.handle).close();
        for _ in 0..self.maximum_polls {
            self.poll()?;
            let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if matches!(
                socket.state(),
                tcp::State::Closed | tcp::State::FinWait2 | tcp::State::TimeWait
            ) {
                socket.abort();
                return Ok((self.adapter.into_ready(), self.polls));
            }
        }
        Err(VirtioTcpError::Timeout)
    }

    fn poll(&mut self) -> Result<(), VirtioTcpError> {
        self.polls = self.polls.checked_add(1).ok_or(VirtioTcpError::Timeout)?;
        let now_millis = match self.deadline {
            Some(deadline) => deadline.elapsed_millis().ok_or(VirtioTcpError::Timeout)?,
            None => i64::from(self.polls / PROOF_POLLS_PER_PROTOCOL_TICK),
        };
        self.interface.poll(
            Instant::from_millis(now_millis),
            &mut self.adapter,
            &mut self.sockets,
        );
        self.adapter
            .take_error()
            .map_or(Ok(()), |error| Err(VirtioTcpError::Device(error)))
    }

    fn poll_io(&mut self) -> Result<(), VirtioTcpIoError> {
        self.poll().map_err(VirtioTcpIoError)
    }
}

impl ErrorType for VirtioTcpStream<'_> {
    type Error = VirtioTcpIoError;
}

impl Read for VirtioTcpStream<'_> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
        if output.is_empty() {
            return Ok(0);
        }
        // An admitted wall-clock deadline bounds product response waiting.
        // A tight CPU poll count can expire before a bounded owner action has
        // completed, even though its response window remains open. Clockless
        // proof streams retain the explicit poll limit. poll() also checks the
        // cumulative u32 counter, so every path retains a finite work bound.
        let read_poll_limit = if self.deadline.is_some() {
            u32::MAX
        } else {
            self.maximum_polls
        };
        for _ in 0..read_poll_limit {
            self.poll_io().inspect_err(|error| {
                report_read_refusal(error.0.as_str());
            })?;
            let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if socket.can_recv() {
                return socket
                    .recv_slice(output)
                    .map_err(|_| VirtioTcpIoError(VirtioTcpError::ReceiveRefused));
            }
            if !socket.may_recv() || !socket.is_active() {
                report_read_refusal("tcp-peer-closed");
                return Ok(0);
            }
        }
        report_read_refusal("tcp-read-poll-bound");
        Err(VirtioTcpIoError(VirtioTcpError::Timeout))
    }
}

fn report_read_refusal(code: &str) {
    crate::arch::early_write(b"CONDUIT_NATIVE_TCP_READ {\"code\":\"");
    crate::arch::early_write(code.as_bytes());
    crate::arch::early_write(b"\"}\n");
}

impl Write for VirtioTcpStream<'_> {
    fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
        if input.is_empty() {
            return Ok(0);
        }
        for _ in 0..self.maximum_polls {
            self.poll_io()?;
            let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if socket.can_send() {
                return socket
                    .send_slice(input)
                    .map_err(|_| VirtioTcpIoError(VirtioTcpError::SendRefused));
            }
            if !socket.may_send() || !socket.is_active() {
                return Err(VirtioTcpIoError(VirtioTcpError::RemoteClosed));
            }
        }
        Err(VirtioTcpIoError(VirtioTcpError::Timeout))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        for _ in 0..self.maximum_polls {
            self.poll_io()?;
            let socket = self.sockets.get::<tcp::Socket>(self.handle);
            if socket.send_queue() == 0 {
                return Ok(());
            }
            if !socket.is_active() {
                return Err(VirtioTcpIoError(VirtioTcpError::RemoteClosed));
            }
        }
        Err(VirtioTcpIoError(VirtioTcpError::Timeout))
    }
}

impl From<VirtioNetError> for VirtioTcpIoError {
    fn from(error: VirtioNetError) -> Self {
        Self(VirtioTcpError::Device(error))
    }
}
