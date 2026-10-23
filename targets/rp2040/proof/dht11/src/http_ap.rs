//! Fixed-capacity DHCP and HTTP service over the existing AP network base.
use conduit_rp2040_network_realization::{answer_appliance_dhcp, DhcpLeasePool};
use core::{
    future::{poll_fn, Future},
    task::Poll,
};
use embassy_futures::select::select;
use embassy_net::{
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
    IpEndpoint, Ipv4Address, Stack,
};
use embassy_time::{with_timeout, Duration, Instant};

#[derive(Clone, Copy)]
pub enum ServiceReady {
    Dhcp,
    Http0,
    Http1,
}
/// Ready is emitted only after actual socket binding/listening. Dropping this
/// future retires all three sockets and their in-flight request futures.
pub async fn serve_acknowledged(
    stack: Stack<'static>,
    ready: impl Fn(ServiceReady),
) -> Result<(), ()> {
    let _ = select(
        dhcp(stack, &ready),
        select(
            http(stack, ServiceReady::Http0, &ready),
            http(stack, ServiceReady::Http1, &ready),
        ),
    )
    .await;
    Err(())
}
async fn dhcp(stack: Stack<'static>, ready: &impl Fn(ServiceReady)) -> Result<(), ()> {
    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx = [0; 576];
    let mut tx = [0; 576];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx, &mut tx_meta, &mut tx);
    socket.bind(67).map_err(|_| ())?;
    ready(ServiceReady::Dhcp);
    let mut leases = DhcpLeasePool::default();
    loop {
        let mut request = [0; 576];
        let mut response = [0; 576];
        if let Ok((len, _)) = socket.recv_from(&mut request).await {
            if let Ok(answer) = answer_appliance_dhcp(&request[..len], &mut leases, &mut response) {
                let _ = with_timeout(
                    Duration::from_secs(2),
                    socket.send_to(
                        &response[..answer.len],
                        IpEndpoint::new(Ipv4Address::BROADCAST.into(), 68),
                    ),
                )
                .await;
            }
        }
    }
}
async fn http(
    stack: Stack<'static>,
    service: ServiceReady,
    ready: &impl Fn(ServiceReady),
) -> Result<(), ()> {
    let mut rx = [0; 512];
    let mut tx = [0; 2048];
    let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
    socket.set_timeout(Some(Duration::from_secs(5)));
    let mut announced = false;
    loop {
        let result = {
            let mut accept = core::pin::pin!(socket.accept(80));
            poll_fn(|cx| {
                let status = accept.as_mut().poll(cx);
                if !announced && !matches!(&status, Poll::Ready(Err(_))) {
                    ready(service);
                    announced = true;
                }
                status
            })
            .await
        };
        result.map_err(|_| ())?;
        {
            let _ = with_timeout(Duration::from_secs(5), exchange(&mut socket)).await;
            socket.close();
            let _ = with_timeout(Duration::from_secs(1), socket.flush()).await;
        }
        socket.abort();
    }
}
async fn exchange(socket: &mut TcpSocket<'_>) -> Result<(), ()> {
    conduit_environment_http::exchange::exchange(socket, || Instant::now().as_millis(), None)
        .await
        .map(|_| ())
        .map_err(|_| ())
}
