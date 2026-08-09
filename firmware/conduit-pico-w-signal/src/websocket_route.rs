//! One fixed, attachment-dependent WebSocket Base and current Session handshake.

use embassy_futures::select::{select, Either};
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::Duration;
use conduit_core::BootId;

use crate::network_receipts::WebSocketRouteIdentity;
use crate::continuable_signal::ContinuableSignalSink;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb_link::UsbLinkSession;

const ATTACHMENT_ID: &str = "r1/pico-network-attachment-1";
const TCP_BUFFER_BYTES: usize = conduit_net::R1_MAXIMUM_FRAME_BYTES as usize + 256;

#[derive(Debug)]
pub struct WebSocketUnavailable;

pub async fn run(
    stack: Stack<'static>,
    link: &mut UsbLinkSession,
    clue: &mut UsbCdc,
    control: &mut cyw43::Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
    continuation: &mut Option<ContinuableSignalSink>,
) -> Result<(), WebSocketUnavailable> {
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
    let plan_c = await_query(link).await.map_err(|_| WebSocketUnavailable)?;
    let execution = if plan_c {
        crate::plan_c_signal_image::execution_identity()
    } else {
        crate::signal_execution_identity::SignalExecutionIdentity::plan_a()
    };
    let signal_runtime = runtime.for_plan(execution.plan_id, execution.host_id);
    let mut plan_a_state;
    let state = if plan_c {
        let state = match ContinuableSignalSink::new(&signal_runtime) {
            Ok(state) => state,
            Err(_) => remain_bootsel(link).await,
        };
        *continuation = Some(state);
        continuation
            .as_mut()
            .expect("Plan C state was installed before carrier activation")
    } else {
        plan_a_state = ContinuableSignalSink::new_plan_a(&signal_runtime)
            .map_err(|_| WebSocketUnavailable)?;
        &mut plan_a_state
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
    let binding = state.binding();
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
        crate::websocket_signal::run(
            &mut socket,
            &mut transport,
            control,
            clue,
            &signal_runtime,
            state,
        ),
    )
    .await {
        Either::First(_) => unreachable!(),
        Either::Second(Ok(())) => {
            socket.abort();
            remain_bootsel(link).await
        }
        Either::Second(Err(_)) => {
            socket.abort();
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
