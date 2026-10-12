//! Shared preparation for the existing smoltcp session; no authority is issued here.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_session<'a>(
    device: &mut (impl Device + ?Sized),
    random_seed: u64,
    mac: [u8; 6],
    endpoint: VirtioTcpEndpoint,
    timestamp: Instant,
    receive_storage: &'a mut [u8],
    transmit_storage: &'a mut [u8],
    socket_storage: &'a mut [SocketStorage<'a>],
) -> Result<(Interface, SocketSet<'a>, smoltcp::iface::SocketHandle), VirtioTcpError> {
    if receive_storage.is_empty() || transmit_storage.is_empty() || socket_storage.len() != 1 {
        return Err(VirtioTcpError::InvalidEndpoint);
    }
    // Refuse occupied borrowed storage before SocketSet::add can panic.
    let mut sockets = SocketSet::new(socket_storage);
    if sockets.iter().next().is_some() {
        return Err(VirtioTcpError::InvalidEndpoint);
    }
    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(mac)));
    config.random_seed = random_seed;
    let mut interface = Interface::new(config, device, timestamp);
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

    Ok((interface, sockets, handle))
}
