//! Retained fixed-storage exchange over a caller-owned admitted Ethernet device.
//! No discovery, authority, retry, TLS or class policy lives in this adapter.
use super::*;
use smoltcp::iface::SocketHandle;

#[derive(Clone, Copy)]
pub struct TcpExchangeOptions {
    pub random_seed: u64,
    pub mac: [u8; 6],
    pub endpoint: VirtioTcpEndpoint,
    pub maximum_polls: u32,
    pub timestamp: Instant,
}

pub struct TcpExchange<'a> {
    interface: Interface,
    sockets: SocketSet<'a>,
    handle: SocketHandle,
    request: &'a [u8],
    response: &'a mut [u8],
    expected_response_bytes: usize,
    maximum_polls: u32,
    polls: u32,
    timestamp: Instant,
    connected: bool,
    request_sent: bool,
    received: usize,
    closing: bool,
    terminal: Option<Result<VirtioTcpReceipt, VirtioTcpError>>,
}

impl<'a> TcpExchange<'a> {
    /// Caller retains the same Device and separately admitted configuration/connect
    /// authority for the lifetime of this session. All storage is borrowed before Play.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        device: &mut (impl Device + ?Sized),
        options: TcpExchangeOptions,
        request: &'a [u8],
        response: &'a mut [u8],
        expected_response_bytes: usize,
        receive_storage: &'a mut [u8],
        transmit_storage: &'a mut [u8],
        socket_storage: &'a mut [SocketStorage<'a>],
    ) -> Result<Self, VirtioTcpError> {
        validate(
            options.endpoint,
            request,
            response,
            expected_response_bytes,
            options.maximum_polls,
        )?;
        let caps = device.capabilities();
        if caps.medium != Medium::Ethernet
            || caps.max_transmission_unit < ETHERNET_FRAME_BYTES
            || options.mac == [0; 6]
            || options.mac[0] & 1 != 0
            || transmit_storage.len() < request.len()
            || receive_storage.len() < expected_response_bytes
        {
            return Err(VirtioTcpError::InvalidEndpoint);
        }
        let (interface, sockets, handle) = session::prepare_session(
            device,
            options.random_seed,
            options.mac,
            options.endpoint,
            options.timestamp,
            receive_storage,
            transmit_storage,
            socket_storage,
        )?;
        Ok(Self {
            interface,
            sockets,
            handle,
            request,
            response,
            expected_response_bytes,
            maximum_polls: options.maximum_polls,
            polls: 0,
            timestamp: options.timestamp,
            connected: false,
            request_sent: false,
            received: 0,
            closing: false,
            terminal: None,
        })
    }

    /// Processes at most one received frame and one bounded socket egress pass.
    /// The caller supplies its actual admitted clock, checks Device errors, and
    /// interleaves frame Host Calls between these steps. No poll allocates storage.
    pub fn poll_step(
        &mut self,
        device: &mut (impl Device + ?Sized),
        timestamp: Instant,
    ) -> Result<Option<VirtioTcpReceipt>, VirtioTcpError> {
        self.drive(device, timestamp, false)
    }

    pub(super) fn poll_legacy(
        &mut self,
        device: &mut (impl Device + ?Sized),
        timestamp: Instant,
    ) -> Result<Option<VirtioTcpReceipt>, VirtioTcpError> {
        self.drive(device, timestamp, true)
    }

    /// Abort local protocol state. Physical-device retirement remains the owner's duty.
    pub fn cancel(&mut self) {
        if self.terminal.is_none() {
            self.sockets.get_mut::<tcp::Socket>(self.handle).abort();
            self.terminal = Some(Err(VirtioTcpError::Cancelled));
        }
    }

    pub fn received(&self) -> &[u8] {
        &self.response[..self.received]
    }
    pub fn polls(&self) -> u32 {
        self.polls
    }

    fn drive(
        &mut self,
        device: &mut (impl Device + ?Sized),
        timestamp: Instant,
        legacy: bool,
    ) -> Result<Option<VirtioTcpReceipt>, VirtioTcpError> {
        if let Some(result) = self.terminal {
            return result.map(Some);
        }
        if timestamp < self.timestamp {
            self.terminal = Some(Err(VirtioTcpError::ClockRegressed));
            self.sockets.get_mut::<tcp::Socket>(self.handle).abort();
            return Err(VirtioTcpError::ClockRegressed);
        }
        self.timestamp = timestamp;
        self.polls += 1;
        if legacy {
            self.interface.poll(timestamp, device, &mut self.sockets);
        } else {
            self.interface.poll_maintenance(timestamp);
            self.interface
                .poll_ingress_single(timestamp, device, &mut self.sockets);
            self.interface
                .poll_egress(timestamp, device, &mut self.sockets);
        }
        let result = self.advance();
        let result = match result {
            Ok(None) if self.polls == self.maximum_polls => Err(VirtioTcpError::Timeout),
            other => other,
        };
        if let Err(error) = result {
            self.sockets.get_mut::<tcp::Socket>(self.handle).abort();
            self.terminal = Some(Err(error));
        } else if let Ok(Some(receipt)) = result {
            self.terminal = Some(Ok(receipt));
        }
        result
    }

    fn advance(&mut self) -> Result<Option<VirtioTcpReceipt>, VirtioTcpError> {
        let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
        self.connected |= socket.state() == tcp::State::Established;
        if self.connected && !self.request_sent && socket.can_send() {
            if socket
                .send_slice(self.request)
                .map_err(|_| VirtioTcpError::SendRefused)?
                != self.request.len()
            {
                return Err(VirtioTcpError::SendRefused);
            }
            self.request_sent = true;
        }
        if socket.can_recv() && self.received < self.expected_response_bytes {
            let count = socket
                .recv_slice(&mut self.response[self.received..self.expected_response_bytes])
                .map_err(|_| VirtioTcpError::ReceiveRefused)?;
            self.received += count;
        }
        if self.received == self.expected_response_bytes && !self.closing {
            socket.close();
            self.closing = true;
        }
        // Completion owns only the local finite session. FinWait2 proves the
        // peer acknowledged our FIN; waiting indefinitely for an optional
        // peer FIN would turn remote behavior into hidden unbounded work.
        if self.closing
            && matches!(
                socket.state(),
                tcp::State::Closed | tcp::State::FinWait2 | tcp::State::TimeWait
            )
        {
            socket.abort();
            return Ok(Some(VirtioTcpReceipt {
                polls: self.polls,
                transmitted_bytes: self.request.len() as u16,
                received_bytes: self.received as u16,
            }));
        }
        if !socket.is_active() && !self.closing {
            return Err(VirtioTcpError::RemoteClosed);
        }

        Ok(None)
    }
}
