//! Opaque selected-peer TCP bytes, with bounded ownership and USB cancellation.
extern crate alloc;
use super::{Attachment, Request, REQUEST};
use crate::{protocol, Cdc};
use alloc::vec::Vec;
use core::cell::Cell;
use embassy_net::{tcp::TcpSocket, Ipv4Address, Stack};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{with_timeout, Duration};
const CHUNK: usize = protocol::CHUNK;
pub(super) struct Transfer {
    pub token: u64,
    pub bytes: Vec<u8>,
}
impl Drop for Transfer {
    fn drop(&mut self) {
        protocol::wipe(&mut self.bytes)
    }
}
struct Chunk {
    token: u64,
    length: usize,
    bytes: [u8; CHUNK],
    done: Option<u32>,
    error: Option<&'static str>,
}
impl Drop for Chunk {
    fn drop(&mut self) {
        protocol::wipe(&mut self.bytes)
    }
}
static STREAM: Channel<CriticalSectionRawMutex, Chunk, 1> = Channel::new();
static NEXT: critical_section::Mutex<Cell<u64>> = critical_section::Mutex::new(Cell::new(0));
static CANCELLED: critical_section::Mutex<Cell<u64>> = critical_section::Mutex::new(Cell::new(0));
fn cancelled(token: u64) -> bool {
    critical_section::with(|cs| CANCELLED.borrow(cs).get() == token)
}
pub(super) fn current(stack: Stack<'static>) -> Option<([u8; 4], u8)> {
    stack
        .is_link_up()
        .then(|| {
            stack
                .config_v4()
                .map(|c| (c.address.address().octets(), c.address.prefix_len()))
        })
        .flatten()
}
pub(super) async fn receive(cdc: &mut Cdc, id: u32, payload: &[u8]) -> Result<(), &'static str> {
    if !critical_section::with(|cs| super::STATUS.borrow(cs).borrow().ready) {
        return Err("radio-not-ready");
    }
    if payload.is_empty() || payload.len() > protocol::MAX_REQUEST {
        return Err("http-request-capacity");
    }
    let token = critical_section::with(|cs| {
        let next = NEXT
            .borrow(cs)
            .get()
            .checked_add(1)
            .ok_or("http-generation-exhausted")?;
        NEXT.borrow(cs).set(next);
        Ok::<_, &'static str>(next)
    })?;
    while STREAM.try_receive().is_ok() {}
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(payload.len())
        .map_err(|_| "http-request-memory")?;
    bytes.extend_from_slice(payload);
    REQUEST.signal(Request::Http(Transfer { token, bytes }));
    let result = with_timeout(Duration::from_secs(32), async {
        loop {
            let chunk = STREAM.receive().await;
            if chunk.token != token {
                continue;
            }
            if let Some(error) = chunk.error {
                return Err(error);
            }
            if let Some(total) = chunk.done {
                return crate::send(cdc, protocol::DONE, id, &total.to_le_bytes())
                    .await
                    .map_err(|_| "usb-disconnected");
            }
            if !cdc.dtr() {
                return Err("usb-disconnected");
            }
            crate::send(cdc, protocol::DATA, id, &chunk.bytes[..chunk.length])
                .await
                .map_err(|_| "usb-disconnected")?;
        }
    })
    .await
    .map_err(|_| "http-transfer-timeout")
    .and_then(|r| r);
    if result.is_err() {
        critical_section::with(|cs| CANCELLED.borrow(cs).set(token));
        while STREAM.try_receive().is_ok() {}
    }
    result
}
async fn emit(chunk: Chunk) -> Result<(), &'static str> {
    if cancelled(chunk.token) {
        return Err("usb-disconnected");
    };
    with_timeout(Duration::from_secs(1), STREAM.send(chunk))
        .await
        .map_err(|_| "usb-consumer-timeout")
}
fn check(
    stack: Stack<'static>,
    attachment: &Attachment,
    token: u64,
) -> Result<[u8; 4], &'static str> {
    if cancelled(token) {
        return Err("usb-disconnected");
    }
    if super::attachment_snapshot().generation != attachment.generation {
        return Err("peer-attachment-changed");
    }
    let peer = attachment.peer.ok_or("peer-not-selected")?;
    if !current(stack).is_some_and(|(ip, prefix)| peer.matches(ip, prefix, attachment.generation)) {
        return Err("peer-attachment-changed");
    }
    Ok(peer.address)
}
pub(super) async fn serve(stack: Stack<'static>, attachment: &Attachment, request: Transfer) {
    let token = request.token;
    let mut rx = protocol::Scratch::<4096>::default();
    let mut tx = protocol::Scratch::<2048>::default();
    let mut socket = TcpSocket::new(stack, &mut rx.0, &mut tx.0);
    socket.set_timeout(Some(Duration::from_secs(5)));
    let result = with_timeout(Duration::from_secs(30), async {
        let peer = check(stack, attachment, token)?;
        with_timeout(
            Duration::from_secs(5),
            socket.connect((Ipv4Address::from_octets(peer), 80)),
        )
        .await
        .map_err(|_| "tcp-connect-timeout")?
        .map_err(|_| "tcp-connect")?;
        let mut at = 0;
        while at < request.bytes.len() {
            check(stack, attachment, token)?;
            let n = with_timeout(Duration::from_secs(5), socket.write(&request.bytes[at..]))
                .await
                .map_err(|_| "tcp-write-timeout")?
                .map_err(|_| "tcp-write")?;
            if n == 0 {
                return Err("tcp-write-zero");
            };
            at += n;
        }
        let mut total = 0usize;
        loop {
            check(stack, attachment, token)?;
            let mut chunk = Chunk {
                token,
                length: 0,
                bytes: [0; CHUNK],
                done: None,
                error: None,
            };
            let n = with_timeout(Duration::from_secs(5), socket.read(&mut chunk.bytes))
                .await
                .map_err(|_| "tcp-read-timeout")?
                .map_err(|_| "tcp-read")?;
            if n == 0 {
                break;
            };
            total = total
                .checked_add(n)
                .filter(|n| *n <= protocol::MAX_RESPONSE)
                .ok_or("http-response-capacity")?;
            chunk.length = n;
            emit(chunk).await?;
        }
        if total == 0 {
            return Err("empty-response");
        };
        check(stack, attachment, token)?;
        Ok(total as u32)
    })
    .await
    .map_err(|_| "http-deadline")
    .and_then(|r| r);
    // Abort is local socket retirement, not radio/scan owner retirement.
    socket.abort();
    drop(socket);
    drop(request);
    let mut final_chunk = Chunk {
        token,
        length: 0,
        bytes: [0; CHUNK],
        done: None,
        error: None,
    };
    match result {
        Ok(total) => final_chunk.done = Some(total),
        Err(error) => final_chunk.error = Some(error),
    };
    let _ = emit(final_chunk).await;
}

pub(super) async fn unavailable(request: Transfer) {
    let token = request.token;
    drop(request);
    let _ = emit(Chunk {
        token,
        length: 0,
        bytes: [0; CHUNK],
        done: None,
        error: Some("radio-unavailable"),
    })
    .await;
}
