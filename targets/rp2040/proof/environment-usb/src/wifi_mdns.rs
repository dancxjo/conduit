//! One bounded mDNS observation of the already selected carrier peer.
use super::{attachment_snapshot, transport, Request, REQUEST};
use core::{cell::Cell, fmt::Write};
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    IpAddress, Ipv4Address, Stack,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{with_timeout, Duration};
#[path = "wifi_mdns_query.rs"]
mod query;
const MAX: usize = 8 + crate::protocol::bench_mdns::MAX_HOST;
pub(super) struct Observation {
    token: u64,
    bytes: [u8; MAX],
    length: usize,
}
static CANCELLED: critical_section::Mutex<Cell<u64>> = critical_section::Mutex::new(Cell::new(0));
static NEXT: critical_section::Mutex<Cell<u64>> = critical_section::Mutex::new(Cell::new(0));
static RESULT: Signal<CriticalSectionRawMutex, (u64, Result<heapless::String<768>, &'static str>)> =
    Signal::new();
pub(super) async fn receive(
    cdc: &mut crate::Cdc,
    id: u32,
    bytes: &[u8],
) -> Result<(), &'static str> {
    query::parse(bytes)?;
    if !attachment_snapshot().ready() {
        return Err("peer-not-selected");
    }
    let token = critical_section::with(|cs| {
        let next = NEXT
            .borrow(cs)
            .get()
            .checked_add(1)
            .ok_or("mdns-generation-exhausted")?;
        NEXT.borrow(cs).set(next);
        Ok::<_, &'static str>(next)
    })?;
    let mut request = Observation {
        token,
        bytes: [0; MAX],
        length: bytes.len(),
    };
    request.bytes[..bytes.len()].copy_from_slice(bytes);
    RESULT.reset();
    REQUEST.signal(Request::Mdns(request));
    let result = with_timeout(Duration::from_secs(10), async {
        loop {
            if !cdc.dtr() {
                return Err("usb-disconnected");
            }
            if let Ok((received, result)) =
                with_timeout(Duration::from_millis(50), RESULT.wait()).await
            {
                if received == token {
                    return result;
                }
            }
        }
    })
    .await
    .map_err(|_| "mdns-owner-timeout")
    .and_then(|r| r);
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            critical_section::with(|cs| CANCELLED.borrow(cs).set(token));
            return Err(error);
        }
    };
    if !cdc.dtr() {
        critical_section::with(|cs| CANCELLED.borrow(cs).set(token));
        return Err("usb-disconnected");
    }
    crate::send(cdc, crate::protocol::INFORMATION, id, result.as_bytes())
        .await
        .map_err(|_| "usb-disconnected")
}
pub(super) fn unavailable(request: Observation) {
    RESULT.signal((request.token, Err("radio-unavailable")))
}
pub(super) async fn serve(stack: Stack<'static>, request: Observation) {
    let result = observe(stack, request.token, &request.bytes[..request.length]).await;
    RESULT.signal((request.token, result));
}
async fn observe(
    stack: Stack<'static>,
    token: u64,
    bytes: &[u8],
) -> Result<heapless::String<768>, &'static str> {
    let query = query::parse(bytes)?;
    let attachment = attachment_snapshot();
    let peer = attachment.peer.ok_or("peer-not-selected")?;
    let check = || {
        if critical_section::with(|cs| CANCELLED.borrow(cs).get() == token) {
            return Err("usb-disconnected");
        }
        if attachment_snapshot().generation != attachment.generation
            || !transport::current(stack)
                .is_some_and(|(ip, prefix)| peer.matches(ip, prefix, attachment.generation))
        {
            Err("mdns-attachment-changed")
        } else {
            Ok(())
        }
    };
    check()?;
    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx = [0; 512];
    let mut tx = [0; 512];
    let mut packet = [0; 512];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx, &mut tx_meta, &mut tx);
    socket.bind(0).map_err(|_| "mdns-bind")?;
    with_timeout(Duration::from_secs(3), async {
        check()?;
        let length = query.packet(&mut packet);
        socket
            .send_to(&packet[..length], (Ipv4Address::new(224, 0, 0, 251), 5353))
            .await
            .map_err(|_| "mdns-send")?;
        for _ in 0..16 {
            check()?;
            let (length, metadata) = socket
                .recv_from(&mut packet)
                .await
                .map_err(|_| "mdns-receive")?;
            check()?;
            let IpAddress::Ipv4(source) = metadata.endpoint.addr;
            if query
                .answer(
                    &packet[..length],
                    source.octets(),
                    metadata.endpoint.port,
                    peer.address,
                )
                .is_ok()
            {
                return Ok(());
            }
        }
        Err("mdns-packet-limit")
    })
    .await
    .map_err(|_| "mdns-timeout")??;
    drop(socket);
    check()?;
    let mut nonce = heapless::String::<16>::new();
    for b in query.nonce {
        write!(&mut nonce, "{b:02x}").map_err(|_| "mdns-status-capacity")?;
    }
    let ip = peer.address;
    let mut out = heapless::String::<768>::new();
    write!(&mut out,"{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"observation\":\"selected-peer-mdns-a\",\"nonce\":\"{}\",\"query_id\":{},\"hostname\":\"{}\",\"address\":[{},{},{},{}],\"source\":[{},{},{},{}],\"source_port\":5353,\"attachment_generation\":{},\"body_member\":false}}",crate::config::ROLE,crate::protocol::BUILD_ID,crate::config::CONFIGURATION_ID,nonce,query.id(),query.host,ip[0],ip[1],ip[2],ip[3],ip[0],ip[1],ip[2],ip[3],attachment.generation).map_err(|_|"mdns-status-capacity")?;
    Ok(out)
}
