//! Fixed-capacity DHCP and HTTP service over the existing AP network base.
use conduit_rp2040_network_realization::{answer_appliance_dhcp, DhcpLeasePool};
use embassy_futures::select::select;
use embassy_net::{
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
    IpEndpoint, Ipv4Address, Stack,
};
use embassy_time::{with_timeout, Duration, Instant};

pub async fn serve(stack: Stack<'static>) -> Result<(), ()> {
    // Any reader failure retires the entire service and its sibling sockets.
    let _ = select(dhcp(stack), select(http(stack), http(stack))).await;
    Err(())
}
async fn dhcp(stack: Stack<'static>) {
    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx = [0; 576];
    let mut tx = [0; 576];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx, &mut tx_meta, &mut tx);
    socket.bind(67).unwrap();
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
                        IpEndpoint::new(Ipv4Address::from(answer.destination).into(), 68),
                    ),
                )
                .await;
            }
        }
    }
}
async fn http(stack: Stack<'static>) -> Result<(), ()> {
    let mut rx = [0; 512];
    let mut tx = [0; 2048];
    let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
    socket.set_timeout(Some(Duration::from_secs(5)));
    loop {
        if socket.accept(80).await.is_ok() {
            let _ = with_timeout(Duration::from_secs(5), exchange(&mut socket)).await;
        }
        let close_deadline = Instant::now() + Duration::from_secs(1);
        socket.close();
        // Reserve half of the unchanged close budget for flushing the reset.
        // A peer withholding its FIN acknowledgment cannot spend that reserve.
        let _ = with_timeout(Duration::from_millis(500), socket.flush()).await;
        socket.abort();
        // abort queues a reset; flush it before accepting another connection.
        // Both phases spend the same absolute close budget. A failure retires
        // this service rather than reusing a socket with outstanding retirement.
        let retired = with_timeout(
            close_deadline.saturating_duration_since(Instant::now()),
            socket.flush(),
        )
        .await
        .is_ok_and(|result| result.is_ok());
        if !retired {
            return Err(());
        }
    }
}
async fn exchange(socket: &mut TcpSocket<'_>) -> Result<(), ()> {
    let mut request = [0; 512];
    let mut used = 0;
    loop {
        let count = socket.read(&mut request[used..]).await.map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        used += count;
        if request[..used].windows(4).any(|v| v == b"\r\n\r\n") {
            break;
        }
        if used == request.len() {
            return Err(());
        }
    }
    let response = if request[..used].starts_with(b"GET / HTTP/1.") {
        crate::http_view::response(Instant::now().as_millis(), false)
    } else if request[..used].starts_with(b"GET /reading HTTP/1.") {
        crate::http_view::response(Instant::now().as_millis(), true)
    } else {
        heapless::String::try_from(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .unwrap()
    };
    let mut written = 0;
    while written < response.len() {
        let count = socket
            .write(&response.as_bytes()[written..])
            .await
            .map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        written += count;
    }
    socket.flush().await.map_err(|_| ())
}
