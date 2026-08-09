//! One fixed, attachment-dependent WebSocket Base and current Session handshake.

use embassy_futures::select::{select, Either};
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::Duration;
use conduit_core::LineOffer;

use crate::network_receipts::WebSocketRouteIdentity;
use crate::continuable_signal::ContinuableSignalSink;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb_link::UsbLinkSession;

const ATTACHMENT_ID: &str = "r1/pico-network-attachment-1";
const TCP_BUFFER_BYTES: usize = conduit_net::R1_MAXIMUM_FRAME_BYTES as usize + 256;

#[derive(Debug)]
pub struct WebSocketUnavailable;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    stack: Stack<'static>,
    link: &mut UsbLinkSession,
    sign: &mut UsbCdc,
    control: &mut cyw43::Control<'_>,
    plan_a_runtime: &RuntimeTranscriptIdentity,
    plan_c_runtime: &RuntimeTranscriptIdentity,
    route_basis: &[LineOffer; 2],
    plan_a_state: &mut Option<ContinuableSignalSink>,
    plan_c_state: &mut Option<ContinuableSignalSink>,
    continuation: &mut Option<ContinuableSignalSink>,
) -> Result<(), WebSocketUnavailable> {
    let Some(config) = stack.config_v4() else {
        remain_bootsel(link).await
    };
    let address = config.address.address().octets();
    let [usb_link, websocket_link] = route_basis;
    let identity = WebSocketRouteIdentity {
        firmware_build_id: crate::network_image::FIRMWARE_BUILD_ID,
        attachment_id: ATTACHMENT_ID,
        interface_pool_id: conduit_net::R1_WIFI_STATION_POOL_ID,
        usb_link,
        websocket_link,
        address,
        port: conduit_net::R1_WEBSOCKET_PORT,
        sign_id: conduit_net::R1_WEBSOCKET_ROUTE_SIGN_ID,
    };
    let plan_c = await_query(link).await.map_err(|_| WebSocketUnavailable)?;
    let signal_runtime = if plan_c { plan_c_runtime } else { plan_a_runtime };
    let mut state = if plan_c {
        match plan_c_state.take() {
            Some(state) => state,
            None => remain_bootsel(link).await,
        }
    } else {
        plan_a_state.take().ok_or(WebSocketUnavailable)?
    };
    if link
        .send_raw_stream_frame(conduit_net::R1_WEBSOCKET_BASE_READY)
        .await
        .is_err()
    {
        remain_bootsel(link).await
    }
    let mut frame = [0_u8; 1024];
    match link.receive_raw_stream_frame(&mut frame).await {
        Ok(raw) if raw == conduit_net::R1_WEBSOCKET_ENDPOINT_SIGN_READY => {}
        _ => remain_bootsel(link).await,
    }
    if sign.write_websocket_endpoint(identity).await.is_err() {
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
    let binding = state.binding();
    if sign
        .write_websocket_link(identity, binding.sink_active_play_id.as_str())
        .await
        .is_err()
    {
        socket.abort();
        remain_bootsel(link).await
    }

    match select(
        crate::bootsel::wait_for_request(link),
        crate::websocket_signal::run(
            &mut socket,
            &mut transport,
            control,
            sign,
            signal_runtime,
            &mut state,
        ),
    )
    .await {
        Either::First(_) => unreachable!(),
        Either::Second(Ok(())) => {
            socket.abort();
            remain_bootsel(link).await
        }
        Either::Second(Err(_)) => {
            if plan_c {
                crate::panic_recovery::set_phase(
                    crate::panic_recovery::PanicPhase::PlanCLineFailure,
                );
            }
            socket.abort();
            if plan_c {
                *continuation = Some(state);
            }
            Err(WebSocketUnavailable)
        }
    }
}

async fn await_query(link: &mut UsbLinkSession) -> Result<bool, ()> {
    let mut frame = [0_u8; 1024];
    loop {
        let raw = link.receive_raw_stream_frame(&mut frame).await.map_err(|_| ())?;
        if crate::bootsel::handle_request(link, raw).await.map_err(|_| ())? {
            continue;
        }
        if raw == conduit_net::R1_WEBSOCKET_BASE_QUERY {
            return Ok(false);
        }
        if raw == conduit_net::R1_PLAN_C_WEBSOCKET_BASE_QUERY {
            return Ok(true);
        }
        return Err(());
    }
}

async fn remain_bootsel(link: &mut UsbLinkSession) -> ! {
    loop {
        crate::bootsel::wait_for_request(link).await.ok();
    }
}
