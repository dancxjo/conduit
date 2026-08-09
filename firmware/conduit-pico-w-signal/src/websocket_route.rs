//! One fixed, attachment-dependent WebSocket Base and current Session handshake.

use embassy_futures::select::{select, Either};
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::Duration;
use conduit_core::BootId;

use crate::network_receipts::WebSocketRouteIdentity;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb_link::UsbLinkSession;

const ATTACHMENT_ID: &str = "r1/pico-network-attachment-1";
const TCP_BUFFER_BYTES: usize = conduit_net::R1_MAXIMUM_FRAME_BYTES as usize + 256;

pub async fn run(
    stack: Stack<'static>,
    link: &mut UsbLinkSession,
    clue: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) -> ! {
    let Some(config) = stack.config_v4() else {
        remain_bootsel(link).await
    };
    let address = config.address.address().octets();
    let [usb_link, websocket_link] =
        conduit_net::r1_route_basis(BootId::from(runtime.boot_id()));
    let identity = WebSocketRouteIdentity {
        firmware_build_id: crate::network_image::FIRMWARE_BUILD_ID,
        attachment_id: ATTACHMENT_ID,
        interface_pool_id: conduit_net::R1_WIFI_STATION_POOL_ID,
        usb_link: &usb_link,
        websocket_link: &websocket_link,
        address,
        port: conduit_net::R1_WEBSOCKET_PORT,
        clue_id: conduit_net::R1_WEBSOCKET_ROUTE_CLUE_ID,
    };
    if await_query(link).await.is_err() {
        remain_bootsel(link).await
    }
    if link
        .send_raw_stream_frame(conduit_net::R1_WEBSOCKET_BASE_READY)
        .await
        .is_err()
    {
        remain_bootsel(link).await
    }
    let mut frame = [0_u8; 1024];
    match link.receive_raw_stream_frame(&mut frame).await {
        Ok(raw) if raw == conduit_net::R1_WEBSOCKET_ENDPOINT_CLUE_READY => {}
        _ => remain_bootsel(link).await,
    }
    if clue.write_websocket_endpoint(identity).await.is_err() {
        remain_bootsel(link).await
    }

    let mut tcp_rx = [0_u8; TCP_BUFFER_BYTES];
    let mut tcp_tx = [0_u8; TCP_BUFFER_BYTES];
    let mut socket = TcpSocket::new(stack, &mut tcp_rx, &mut tcp_tx);
    socket.set_timeout(Some(Duration::from_secs(10)));
    if socket.accept(conduit_net::R1_WEBSOCKET_PORT).await.is_err() {
        remain_bootsel(link).await
    }
    let Ok(mut transport) = crate::websocket_transport::WebSocketTransport::accept(&mut socket).await
    else {
        socket.abort();
        remain_bootsel(link).await
    };
    if crate::websocket_session::accept_probe(&mut socket, &mut transport, runtime)
        .await
        .is_err()
    {
        socket.abort();
        remain_bootsel(link).await
    }
    let binding = conduit_net::r1_websocket_probe_binding(BootId::from(runtime.boot_id()));
    if clue
        .write_websocket_link(identity, binding.sink_active_play_id.as_str())
        .await
        .is_err()
    {
        socket.abort();
        remain_bootsel(link).await
    }

    match select(
        crate::bootsel::wait_for_request(link),
        transport.wait_for_disconnect(&mut socket),
    )
    .await
    {
        Either::First(_) => unreachable!(),
        Either::Second(()) => {
            socket.abort();
            remain_bootsel(link).await
        }
    }
}

async fn await_query(link: &mut UsbLinkSession) -> Result<(), ()> {
    let mut frame = [0_u8; 1024];
    loop {
        let raw = link.receive_raw_stream_frame(&mut frame).await.map_err(|_| ())?;
        if crate::bootsel::handle_request(link, raw).await.map_err(|_| ())? {
            continue;
        }
        if raw == conduit_net::R1_WEBSOCKET_BASE_QUERY {
            return Ok(());
        }
        return Err(());
    }
}

async fn remain_bootsel(link: &mut UsbLinkSession) -> ! {
    loop {
        crate::bootsel::wait_for_request(link).await.ok();
    }
}
