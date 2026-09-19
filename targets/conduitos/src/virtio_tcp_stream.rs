//! Blocking fixed-storage TCP stream used beneath reviewed protocol libraries.

use embedded_io::{Error, ErrorKind, ErrorType, Read, Write};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet, SocketStorage},
    socket::tcp,
    time::Instant,
    wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address},
};

use crate::{
    arch::{VirtioNetError, VirtioNetReady},
    virtio_tcp::{VirtioDevice, VirtioTcpEndpoint, VirtioTcpError},
};

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
            VirtioTcpError::InvalidEndpoint => ErrorKind::InvalidInput,
            VirtioTcpError::ConnectRefused => ErrorKind::ConnectionRefused,
            VirtioTcpError::RemoteClosed => ErrorKind::ConnectionAborted,
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
}

impl<'a> VirtioTcpStream<'a> {
    pub(crate) fn connect(
        device: VirtioNetReady,
        random_seed: u64,
        endpoint: VirtioTcpEndpoint,
        maximum_polls: u32,
        receive_storage: &'a mut [u8],
        transmit_storage: &'a mut [u8],
        socket_storage: &'a mut [SocketStorage<'a>],
    ) -> Result<Self, VirtioTcpError> {
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

        let socket = tcp::Socket::new(
            tcp::SocketBuffer::new(receive_storage),
            tcp::SocketBuffer::new(transmit_storage),
        );
        let mut sockets = SocketSet::new(socket_storage);
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
        let mut stream = Self {
            adapter,
            interface,
            sockets,
            handle,
            maximum_polls,
            polls: 0,
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

    pub(crate) fn close_tcp(mut self) -> Result<u32, VirtioTcpError> {
        self.sockets.get_mut::<tcp::Socket>(self.handle).close();
        for _ in 0..self.maximum_polls {
            self.poll()?;
            let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if matches!(
                socket.state(),
                tcp::State::Closed | tcp::State::FinWait2 | tcp::State::TimeWait
            ) {
                socket.abort();
                return Ok(self.polls);
            }
        }
        Err(VirtioTcpError::Timeout)
    }

    fn poll(&mut self) -> Result<(), VirtioTcpError> {
        self.polls = self.polls.checked_add(1).ok_or(VirtioTcpError::Timeout)?;
        self.interface.poll(
            Instant::from_millis(i64::from(self.polls)),
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
        for _ in 0..self.maximum_polls {
            self.poll_io()?;
            let socket = self.sockets.get_mut::<tcp::Socket>(self.handle);
            if socket.can_recv() {
                return socket
                    .recv_slice(output)
                    .map_err(|_| VirtioTcpIoError(VirtioTcpError::ReceiveRefused));
            }
            if !socket.may_recv() || !socket.is_active() {
                return Ok(0);
            }
        }
        Err(VirtioTcpIoError(VirtioTcpError::Timeout))
    }
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

const fn ipv4(address: [u8; 4]) -> Ipv4Address {
    Ipv4Address::new(address[0], address[1], address[2], address[3])
}

impl From<VirtioNetError> for VirtioTcpIoError {
    fn from(error: VirtioNetError) -> Self {
        Self(VirtioTcpError::Device(error))
    }
}
