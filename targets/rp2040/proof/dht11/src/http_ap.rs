//! Fixed-capacity DHCP and HTTP service over the existing AP network base.
use conduit_environment_http::{
    exchange::{CompletedExchange, NetworkSetupMode, ReadingProvider, SelectedNetworkSetup},
    network_setup::{NetworkSetupLimits, NetworkSetupWriter},
};
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

pub type ReadingBuffers = [heapless::String<2048>; 2];

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
    setup: Option<NetworkSetupWriter<'static>>,
    connect: bool,
    ready: impl Fn(ServiceReady),
) -> Result<(), ()> {
    let mut responses = [heapless::String::new(), heapless::String::new()];
    serve_acknowledged_with_reading(stack, setup, connect, None, &mut responses, ready).await
}

/// Root explicitly selects reading availability. This entry retains no Face or
/// sampler and cannot bypass bootstrap through the exchange callback alone.
pub async fn serve_acknowledged_with_reading(
    stack: Stack<'static>,
    setup: Option<NetworkSetupWriter<'static>>,
    connect: bool,
    reading: Option<ReadingProvider>,
    responses: &mut ReadingBuffers,
    ready: impl Fn(ServiceReady),
) -> Result<(), ()> {
    serve_mode_with_reading(stack, setup, connect, reading, responses, true, ready).await
}
/// One shared HTTP future for either interface. The DHCP future remains dormant
/// in station mode; it cannot bind an upstream DHCP socket. Runtime mode avoids
/// retaining duplicate HTTP request frames across AP/station service changes.
pub async fn serve_mode_with_reading(
    stack: Stack<'static>,
    setup: Option<NetworkSetupWriter<'static>>,
    connect: bool,
    reading: Option<ReadingProvider>,
    responses: &mut ReadingBuffers,
    access_point: bool,
    ready: impl Fn(ServiceReady),
) -> Result<(), ()> {
    let address_service = async {
        if access_point {
            dhcp(stack, &ready).await
        } else {
            core::future::pending::<Result<(), ()>>().await
        }
    };
    let _ = select(
        address_service,
        serve_station_with_reading(stack, setup, connect, reading, responses, &ready),
    )
    .await;
    Err(())
}
/// Station mode retains only the two HTTP sockets. It must never serve DHCP on
/// the selected upstream network. Dropping this future retires both requests.
pub async fn serve_station_with_reading(
    stack: Stack<'static>,
    setup: Option<NetworkSetupWriter<'static>>,
    connect: bool,
    reading: Option<ReadingProvider>,
    responses: &mut ReadingBuffers,
    ready: impl Fn(ServiceReady),
) -> Result<(), ()> {
    let [first, second] = responses;
    let _ = select(
        http(
            stack,
            setup.clone(),
            connect,
            reading,
            first,
            ServiceReady::Http0,
            &ready,
        ),
        http(
            stack,
            setup,
            connect,
            reading,
            second,
            ServiceReady::Http1,
            &ready,
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
                        IpEndpoint::new(Ipv4Address::from(answer.destination).into(), 68),
                    ),
                )
                .await;
            }
        }
    }
}
async fn http(
    stack: Stack<'static>,
    setup: Option<NetworkSetupWriter<'static>>,
    connect: bool,
    reading: Option<ReadingProvider>,
    response: &mut heapless::String<2048>,
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
            let deadline = Instant::now() + Duration::from_secs(5);
            // Only credential submissions acquire mutation custody. The returned
            // completion retains it across the bounded close; ordinary reads
            // retain the selected policy without queueing behind this mailbox.
            let custody = with_timeout(
                deadline.saturating_duration_since(Instant::now()),
                exchange(&mut socket, setup.as_ref(), connect, reading, response),
            )
            .await
            .ok()
            .and_then(Result::ok);
            let close_deadline = Instant::now() + Duration::from_secs(1);
            socket.close();
            let _ = with_timeout(
                close_deadline.saturating_duration_since(Instant::now()),
                socket.flush(),
            )
            .await;
            // Finish retirement even when graceful close times out before
            // exposing acknowledged input to another task or reader.
            socket.abort();
            // abort() queues the reset: reusing the socket before its flush
            // completes can discard retirement of the previous connection.
            // Both phases spend the original close budget. Failure retires
            // this service instead of advertising a reusable socket.
            let retired = with_timeout(
                close_deadline.saturating_duration_since(Instant::now()),
                socket.flush(),
            )
            .await
            .is_ok_and(|result| result.is_ok());
            drop(custody);
            if !retired {
                return Err(());
            }
        }
    }
}
async fn exchange<'a>(
    socket: &mut TcpSocket<'_>,
    setup: Option<&'a NetworkSetupWriter<'_>>,
    connect: bool,
    reading: Option<ReadingProvider>,
    response: &mut heapless::String<2048>,
) -> Result<CompletedExchange<'a>, ()> {
    let selection = if let Some(writer) = setup {
        let mode = if connect && reading.is_some() {
            NetworkSetupMode::Ready
        } else if connect {
            NetworkSetupMode::BootstrapControl
        } else {
            NetworkSetupMode::PreparationOnly
        };
        Some(
            SelectedNetworkSetup::new(
                writer,
                NetworkSetupLimits {
                    minimum_credential_bytes: 8,
                    maximum_credential_bytes: 63,
                },
                mode,
            )
            .map_err(|_| ())?,
        )
    } else {
        None
    };
    let selected = reading.unwrap_or(|now, json, after| {
        if after.is_some() {
            None
        } else {
            Some(conduit_environment_http::response(now, json))
        }
    });
    conduit_environment_http::exchange::exchange_selected_setup(
        socket,
        || Instant::now().as_millis(),
        selection,
        response,
        |now, json| selected(now, json, None),
        |after| selected(Instant::now().as_millis(), true, Some(after)),
    )
    .await
    .map_err(|_| ())
}
