//! One fixed-storage IPv4/TCP session over the admitted x86 VirtIO-net device.
//!
//! This layer owns no endpoint discovery, reconnect, TLS, WebSocket, or Line
//! meaning. Its caller supplies one exact endpoint and finite poll budget.

use core::cell::{Cell, RefCell};

use smoltcp::{
    iface::{Config, Interface, SocketSet, SocketStorage},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    socket::tcp,
    time::Instant,
    wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address},
};

use crate::arch::{VirtioNetError, VirtioNetReady};

const ETHERNET_FRAME_BYTES: usize = 1514;
const SOCKET_BUFFER_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioTcpEndpoint {
    pub guest_address: [u8; 4],
    pub prefix_length: u8,
    pub gateway: [u8; 4],
    pub remote_address: [u8; 4],
    pub remote_port: u16,
    pub local_port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioTcpReceipt {
    pub polls: u32,
    pub transmitted_bytes: u16,
    pub received_bytes: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioTcpError {
    InvalidEndpoint,
    RequestTooLarge,
    ResponseTooLarge,
    ConnectRefused,
    SendRefused,
    ReceiveRefused,
    RemoteClosed,
    Timeout,
    Device(VirtioNetError),
}

impl VirtioTcpError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidEndpoint => "virtio-tcp-endpoint-invalid",
            Self::RequestTooLarge => "virtio-tcp-request-too-large",
            Self::ResponseTooLarge => "virtio-tcp-response-too-large",
            Self::ConnectRefused => "virtio-tcp-connect-refused",
            Self::SendRefused => "virtio-tcp-send-refused",
            Self::ReceiveRefused => "virtio-tcp-receive-refused",
            Self::RemoteClosed => "virtio-tcp-remote-closed",
            Self::Timeout => "virtio-tcp-timeout",
            Self::Device(error) => error.as_str(),
        }
    }
}

pub fn exchange(
    device: VirtioNetReady,
    random_seed: u64,
    endpoint: VirtioTcpEndpoint,
    request: &[u8],
    response: &mut [u8],
    expected_response_bytes: usize,
    maximum_polls: u32,
) -> Result<VirtioTcpReceipt, VirtioTcpError> {
    validate(
        endpoint,
        request,
        response,
        expected_response_bytes,
        maximum_polls,
    )?;
    let mac = device.identity().mac;
    let mut adapter = VirtioDevice::new(device);
    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(mac)));
    config.random_seed = random_seed;
    let mut interface = Interface::new(config, &mut adapter, Instant::ZERO);
    let mut address_admitted = false;
    interface.update_ip_addrs(|addresses| {
        address_admitted = addresses
            .push(IpCidr::new(
                IpAddress::Ipv4(ipv4(endpoint.guest_address)),
                endpoint.prefix_length,
            ))
            .is_ok();
    });
    if !address_admitted {
        return Err(VirtioTcpError::InvalidEndpoint);
    }
    interface
        .routes_mut()
        .add_default_ipv4_route(ipv4(endpoint.gateway))
        .map_err(|_| VirtioTcpError::InvalidEndpoint)?;

    let mut receive_storage = [0; SOCKET_BUFFER_BYTES];
    let mut transmit_storage = [0; SOCKET_BUFFER_BYTES];
    let socket = tcp::Socket::new(
        tcp::SocketBuffer::new(&mut receive_storage[..]),
        tcp::SocketBuffer::new(&mut transmit_storage[..]),
    );
    let mut socket_storage = [SocketStorage::EMPTY];
    let mut sockets = SocketSet::new(&mut socket_storage[..]);
    let handle = sockets.add(socket);
    sockets
        .get_mut::<tcp::Socket>(handle)
        .connect(
            interface.context(),
            (
                IpAddress::Ipv4(ipv4(endpoint.remote_address)),
                endpoint.remote_port,
            ),
            endpoint.local_port,
        )
        .map_err(|_| VirtioTcpError::ConnectRefused)?;

    let mut connected = false;
    let mut request_sent = false;
    let mut received = 0usize;
    let mut closing = false;
    for poll in 1..=maximum_polls {
        let timestamp = Instant::from_millis(i64::from(poll));
        interface.poll(timestamp, &mut adapter, &mut sockets);
        if let Some(error) = adapter.take_error() {
            return Err(VirtioTcpError::Device(error));
        }
        let socket = sockets.get_mut::<tcp::Socket>(handle);
        connected |= socket.state() == tcp::State::Established;
        if connected && !request_sent && socket.can_send() {
            if socket
                .send_slice(request)
                .map_err(|_| VirtioTcpError::SendRefused)?
                != request.len()
            {
                return Err(VirtioTcpError::SendRefused);
            }
            request_sent = true;
        }
        if socket.can_recv() && received < expected_response_bytes {
            let count = socket
                .recv_slice(&mut response[received..expected_response_bytes])
                .map_err(|_| VirtioTcpError::ReceiveRefused)?;
            received += count;
        }
        if received == expected_response_bytes && !closing {
            socket.close();
            closing = true;
        }
        // Completion owns only the local finite session. FinWait2 proves the
        // peer acknowledged our FIN; waiting indefinitely for an optional
        // peer FIN would turn remote behavior into hidden unbounded work.
        if closing
            && matches!(
                socket.state(),
                tcp::State::Closed | tcp::State::FinWait2 | tcp::State::TimeWait
            )
        {
            socket.abort();
            return Ok(VirtioTcpReceipt {
                polls: poll,
                transmitted_bytes: request.len() as u16,
                received_bytes: received as u16,
            });
        }
        if !socket.is_active() && !closing {
            return Err(VirtioTcpError::RemoteClosed);
        }
    }
    Err(VirtioTcpError::Timeout)
}

fn validate(
    endpoint: VirtioTcpEndpoint,
    request: &[u8],
    response: &[u8],
    expected_response_bytes: usize,
    maximum_polls: u32,
) -> Result<(), VirtioTcpError> {
    if endpoint.prefix_length == 0
        || endpoint.prefix_length > 32
        || endpoint.remote_port == 0
        || endpoint.local_port == 0
        || endpoint.guest_address == [0; 4]
        || endpoint.gateway == [0; 4]
        || endpoint.remote_address == [0; 4]
        || maximum_polls == 0
    {
        return Err(VirtioTcpError::InvalidEndpoint);
    }
    if request.is_empty() || request.len() > SOCKET_BUFFER_BYTES {
        return Err(VirtioTcpError::RequestTooLarge);
    }
    if expected_response_bytes == 0
        || expected_response_bytes > response.len()
        || expected_response_bytes > SOCKET_BUFFER_BYTES
    {
        return Err(VirtioTcpError::ResponseTooLarge);
    }
    Ok(())
}

const fn ipv4(address: [u8; 4]) -> Ipv4Address {
    Ipv4Address::new(address[0], address[1], address[2], address[3])
}

struct VirtioDevice {
    device: RefCell<VirtioNetReady>,
    receive: [u8; ETHERNET_FRAME_BYTES],
    error: Cell<Option<VirtioNetError>>,
}

impl VirtioDevice {
    fn new(device: VirtioNetReady) -> Self {
        Self {
            device: RefCell::new(device),
            receive: [0; ETHERNET_FRAME_BYTES],
            error: Cell::new(None),
        }
    }

    fn take_error(&self) -> Option<VirtioNetError> {
        self.error.take()
    }
}

impl Device for VirtioDevice {
    type RxToken<'a> = VirtioRxToken<'a>;
    type TxToken<'a> = VirtioTxToken<'a>;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        match self.device.borrow_mut().receive(&mut self.receive) {
            Ok(length) => Some((
                VirtioRxToken {
                    frame: &self.receive[..length],
                },
                VirtioTxToken {
                    device: &self.device,
                    error: &self.error,
                },
            )),
            Err(VirtioNetError::Pressure) => None,
            Err(error) => {
                self.error.set(Some(error));
                None
            }
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(VirtioTxToken {
            device: &self.device,
            error: &self.error,
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = ETHERNET_FRAME_BYTES;
        capabilities
    }
}

struct VirtioRxToken<'a> {
    frame: &'a [u8],
}

impl RxToken for VirtioRxToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.frame)
    }
}

struct VirtioTxToken<'a> {
    device: &'a RefCell<VirtioNetReady>,
    error: &'a Cell<Option<VirtioNetError>>,
}

impl TxToken for VirtioTxToken<'_> {
    fn consume<R, F>(self, length: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut frame = [0; ETHERNET_FRAME_BYTES];
        let usable = length.min(frame.len());
        let result = f(&mut frame[..usable]);
        if length > frame.len() {
            self.error.set(Some(VirtioNetError::FrameTooLarge));
        } else if let Err(error) = self.device.borrow_mut().send(&frame[..length], 1_000_000) {
            self.error.set(Some(error));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> VirtioTcpEndpoint {
        VirtioTcpEndpoint {
            guest_address: [10, 0, 2, 15],
            prefix_length: 24,
            gateway: [10, 0, 2, 2],
            remote_address: [10, 0, 2, 100],
            remote_port: 9000,
            local_port: 49152,
        }
    }

    #[test]
    fn rejects_zero_bounds_and_oversized_fixed_buffers() {
        let response = [0; 4];
        assert_eq!(
            validate(endpoint(), b"x", &response, 1, 0),
            Err(VirtioTcpError::InvalidEndpoint)
        );
        assert_eq!(
            validate(endpoint(), &[], &response, 1, 1),
            Err(VirtioTcpError::RequestTooLarge)
        );
        assert_eq!(
            validate(endpoint(), b"x", &response, 5, 1),
            Err(VirtioTcpError::ResponseTooLarge)
        );
    }
}
