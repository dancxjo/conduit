#![no_std]
//! Nonblocking owner of an actual Embassy socket. Call `poll` from the socket's
//! executor and yield between passes; the blocking C worker runs elsewhere.
use conduit_zenoh_pico_platform::{
    Endpoint, Fault, NetworkCommand, Phase, SocketBridge, SocketGeneration,
};
use core::cell::RefCell;
use embassy_net::{
    tcp::{State, TcpSocket},
    IpEndpoint, Ipv4Address, TryError,
};
mod worker;
pub use worker::{SocketWorker, WorkerRuntime};
mod arena;
pub use arena::{ArenaError, ArenaStats, CArena};
mod callbacks;
pub use callbacks::{CallbackPlatform, Callbacks};
mod exports;

/// Short access to caller-supplied fixed backing. No network call occurs while
/// holding this lock. Implementations must serialize both cores.
pub trait BridgeAccess<const RX: usize, const TX: usize> {
    fn with<R>(&self, f: impl FnOnce(&mut SocketBridge<RX, TX>) -> R) -> R;
}
impl<const RX: usize, const TX: usize> BridgeAccess<RX, TX>
    for critical_section::Mutex<RefCell<SocketBridge<RX, TX>>>
{
    fn with<R>(&self, f: impl FnOnce(&mut SocketBridge<RX, TX>) -> R) -> R {
        critical_section::with(|cs| f(&mut self.borrow(cs).borrow_mut()))
    }
}
struct Connecting {
    generation: SocketGeneration,
    endpoint: Endpoint,
    deadline_ms: u64,
    started: bool,
}
/// Fixed staging only; the actual socket and its separate TCP buffers remain
/// caller-owned. `CHUNK` bounds each pass, not total session capacity.
pub struct SocketOwner<const CHUNK: usize> {
    connecting: Option<Connecting>,
    bytes: [u8; CHUNK],
    endpoints: Option<(SocketGeneration, IpEndpoint, IpEndpoint)>,
}
impl<const CHUNK: usize> Default for SocketOwner<CHUNK> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const CHUNK: usize> SocketOwner<CHUNK> {
    pub const fn new() -> Self {
        assert!(CHUNK > 0);
        Self {
            connecting: None,
            bytes: [0; CHUNK],
            endpoints: None,
        }
    }
    /// Actual endpoints of the last established generation, never guessed from
    /// interface configuration. Consumers still check their bridge generation.
    pub fn endpoints(&self) -> Option<(SocketGeneration, IpEndpoint, IpEndpoint)> {
        self.endpoints
    }
    fn abort(&mut self, socket: &mut TcpSocket<'_>) {
        socket.abort();
        self.connecting = None;
        self.endpoints = None;
        self.bytes.fill(0);
    }
    /// One bounded pass: one connect check, at most one read and one write.
    /// No future is retained, so close can retire without cancelling a borrowed
    /// buffer. `try_write` returns the exact locally accepted prefix.
    pub fn poll<const RX: usize, const TX: usize>(
        &mut self,
        socket: &mut TcpSocket<'_>,
        bridge: &impl BridgeAccess<RX, TX>,
        now_ms: u64,
    ) {
        match bridge.with(|b| b.take_command(now_ms)) {
            Some(NetworkCommand::Close { generation }) => {
                self.abort(socket);
                let _ = bridge.with(|b| b.complete_close(generation));
                return;
            }
            Some(NetworkCommand::Connect {
                generation,
                endpoint,
                deadline_ms,
            }) => {
                self.abort(socket);
                self.connecting = Some(Connecting {
                    generation,
                    endpoint,
                    deadline_ms,
                    started: false,
                });
            }
            None => {}
        }
        if let Some(connecting) = self.connecting.as_mut() {
            let generation = connecting.generation;
            if bridge.with(|b| b.phase()) != Phase::Connecting {
                self.abort(socket);
                return;
            }
            let result = if now_ms >= connecting.deadline_ms {
                Some(Err(Fault::TimedOut))
            } else if !connecting.started {
                connecting.started = true;
                match socket.try_connect((
                    Ipv4Address::from_octets(connecting.endpoint.ipv4),
                    connecting.endpoint.port,
                )) {
                    Ok(()) => Some(Ok(())),
                    Err(TryError::WouldBlock) => None,
                    Err(TryError::Other(_)) => Some(Err(Fault::Network)),
                }
            } else {
                // Calling try_connect again after a refused SYN would silently
                // start another attempt. Observe that same attempt instead.
                match socket.state() {
                    State::SynSent | State::SynReceived => None,
                    State::Established | State::CloseWait => Some(Ok(())),
                    _ => Some(Err(Fault::Network)),
                }
            };
            if let Some(result) = result {
                self.connecting = None;
                if bridge.with(|b| b.complete_connect(generation, now_ms, result))
                    != Ok(Phase::Open)
                {
                    self.abort(socket);
                    return;
                }
                self.endpoints = socket
                    .local_endpoint()
                    .zip(socket.remote_endpoint())
                    .map(|(local, remote)| (generation, local, remote));
            } else {
                return;
            }
        }
        let (generation, phase) = bridge.with(|b| (b.generation(), b.phase()));
        let Some(generation) = generation else {
            self.abort(socket);
            return;
        };
        if phase != Phase::Open {
            self.abort(socket);
            return;
        }
        // Only this owner produces RX. Concurrent consumption increases room;
        // close may invalidate it, in which case accepted bytes are retired.
        let room = bridge
            .with(|b| b.rx_room(generation))
            .unwrap_or(0)
            .min(CHUNK);
        if room != 0 {
            match socket.try_read(&mut self.bytes[..room]) {
                Ok(0) => {
                    let _ = bridge.with(|b| b.peer_closed(generation));
                }
                Ok(n) => {
                    if bridge.with(|b| b.network_receive(generation, &self.bytes[..n])) != Ok(n) {
                        let _ = bridge.with(|b| b.fail(generation, Fault::Network));
                        self.abort(socket);
                        return;
                    }
                }
                Err(TryError::WouldBlock) => {}
                Err(TryError::Other(_)) => {
                    let _ = bridge.with(|b| b.fail(generation, Fault::Network));
                }
            }
            self.bytes.fill(0);
        }
        if bridge.with(|b| b.phase()) != Phase::Open {
            self.abort(socket);
            return;
        }
        let ticket = match bridge.with(|b| b.network_send(generation, &mut self.bytes)) {
            Ok(Some(ticket)) => ticket,
            Ok(None) => return,
            Err(_) => {
                let _ = bridge.with(|b| b.fail(generation, Fault::Network));
                self.abort(socket);
                return;
            }
        };
        match socket.try_write(&self.bytes[..ticket.len()]) {
            Ok(0) | Err(TryError::Other(_)) => {
                let _ = bridge.with(|b| b.fail(generation, Fault::Network));
                self.abort(socket);
            }
            Ok(n) => {
                if bridge.with(|b| b.network_sent(ticket, n)).is_err() {
                    self.abort(socket);
                }
            }
            // WouldBlock guarantees zero acceptance. No awaited write can
            // time out here with an unknown accepted prefix.
            Err(TryError::WouldBlock) => {
                let _ = bridge.with(|b| b.cancel_send(ticket));
            }
        }
        self.bytes.fill(0);
    }
}
