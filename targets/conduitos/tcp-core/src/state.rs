//! One canonical smoltcp IPv4/TCP connection; each poll performs finite work.
use crate::{
    FRAME_BYTES, Refusal, SOCKET_BYTES, buffer,
    device::Adapter,
    wire::{Request, Response},
};
use alloc::{boxed::Box, vec::Vec};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet, SocketStorage},
    socket::tcp,
    time::Instant,
    wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address},
};
/// Seed8, MAC6, prefix1, reserved-zero1, localIPv4/gateway/remoteIPv4 each4,
/// localPort2, remotePort2, initialMillis8; all numeric fields little endian.
pub const INITIALIZE_BYTES: usize = 40;
#[derive(Clone, Copy, Debug)]
pub struct Initialize {
    pub seed: u64,
    pub mac: [u8; 6],
    pub prefix: u8,
    pub local: [u8; 4],
    pub gateway: [u8; 4],
    pub remote: [u8; 4],
    pub local_port: u16,
    pub remote_port: u16,
    pub millis: u64,
}
impl Initialize {
    pub fn encode(self, out: &mut [u8]) -> Result<(), Refusal> {
        self.validate()?;
        let b = out.get_mut(..INITIALIZE_BYTES).ok_or(Refusal::Bounds)?;
        b.fill(0);
        b[..8].copy_from_slice(&self.seed.to_le_bytes());
        b[8..14].copy_from_slice(&self.mac);
        b[14] = self.prefix;
        b[16..20].copy_from_slice(&self.local);
        b[20..24].copy_from_slice(&self.gateway);
        b[24..28].copy_from_slice(&self.remote);
        b[28..30].copy_from_slice(&self.local_port.to_le_bytes());
        b[30..32].copy_from_slice(&self.remote_port.to_le_bytes());
        b[32..40].copy_from_slice(&self.millis.to_le_bytes());
        Ok(())
    }
    fn decode(b: &[u8]) -> Result<Self, Refusal> {
        let b = b.get(..INITIALIZE_BYTES).ok_or(Refusal::Bounds)?;
        if b[15] != 0 {
            return Err(Refusal::Malformed);
        }
        let this = Self {
            seed: u64::from_le_bytes(b[..8].try_into().map_err(|_| Refusal::Malformed)?),
            mac: b[8..14].try_into().map_err(|_| Refusal::Malformed)?,
            prefix: b[14],
            local: b[16..20].try_into().map_err(|_| Refusal::Malformed)?,
            gateway: b[20..24].try_into().map_err(|_| Refusal::Malformed)?,
            remote: b[24..28].try_into().map_err(|_| Refusal::Malformed)?,
            local_port: u16::from_le_bytes([b[28], b[29]]),
            remote_port: u16::from_le_bytes([b[30], b[31]]),
            millis: u64::from_le_bytes(b[32..40].try_into().map_err(|_| Refusal::Malformed)?),
        };
        this.validate()?;
        Ok(this)
    }
    fn validate(self) -> Result<(), Refusal> {
        if self.prefix == 0
            || self.prefix > 32
            || self.local_port == 0
            || self.remote_port == 0
            || self.millis > i64::MAX as u64
        {
            return Err(Refusal::Bounds);
        }
        Ok(())
    }
}
fn ip(a: [u8; 4]) -> Ipv4Address {
    Ipv4Address::new(a[0], a[1], a[2], a[3])
}
struct Engine {
    interface: Interface,
    sockets: SocketSet<'static>,
    handle: SocketHandle,
    adapter: Adapter,
    millis: u64,
    polls: u32,
}
impl Engine {
    fn new(init: Initialize) -> Result<Self, Refusal> {
        let mut adapter = Adapter::new()?;
        let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(init.mac)));
        config.random_seed = init.seed;
        let mut interface = Interface::new(
            config,
            &mut adapter,
            Instant::from_millis(init.millis as i64),
        );
        let mut admitted = false;
        interface.update_ip_addrs(|a| {
            admitted = a
                .push(IpCidr::new(IpAddress::Ipv4(ip(init.local)), init.prefix))
                .is_ok();
        });
        if !admitted {
            return Err(Refusal::Capacity);
        }
        interface
            .routes_mut()
            .add_default_ipv4_route(ip(init.gateway))
            .map_err(|_| Refusal::Bounds)?;
        let mut slots = Vec::new();
        slots.try_reserve_exact(1).map_err(|_| Refusal::Capacity)?;
        slots.push(SocketStorage::EMPTY);
        let mut sockets = SocketSet::new(slots);
        let socket = tcp::Socket::new(
            tcp::SocketBuffer::new(buffer(SOCKET_BYTES)?),
            tcp::SocketBuffer::new(buffer(SOCKET_BYTES)?),
        );
        let handle = sockets.add(socket);
        sockets
            .get_mut::<tcp::Socket>(handle)
            .connect(
                interface.context(),
                (IpAddress::Ipv4(ip(init.remote)), init.remote_port),
                init.local_port,
            )
            .map_err(|_| Refusal::Connect)?;
        Ok(Self {
            interface,
            sockets,
            handle,
            adapter,
            millis: init.millis,
            polls: 0,
        })
    }
    fn status(&self) -> Response {
        let s = self.sockets.get::<tcp::Socket>(self.handle);
        let state = match s.state() {
            tcp::State::Closed => 0,
            tcp::State::Listen => 1,
            tcp::State::SynSent => 2,
            tcp::State::SynReceived => 3,
            tcp::State::Established => 4,
            tcp::State::FinWait1 => 5,
            tcp::State::FinWait2 => 6,
            tcp::State::CloseWait => 7,
            tcp::State::Closing => 8,
            tcp::State::LastAck => 9,
            tcp::State::TimeWait => 10,
        };
        let (flags, frames) = (
            u32::from(s.can_send())
                | u32::from(s.may_send()) << 1
                | u32::from(s.may_recv()) << 2
                | u32::from(s.is_active()) << 3,
            self.adapter.pending(),
        );
        Response::Status {
            state,
            receive: s.recv_queue() as u32,
            send: s.send_queue() as u32,
            frames: frames.0,
            next_frame: frames.1,
            polls: self.polls,
            flags,
        }
    }
}
#[derive(Default)]
pub struct State {
    engine: Option<Box<Engine>>,
}
impl State {
    pub fn execute(&mut self, r: Request, window: &mut [u8]) -> Result<Response, Refusal> {
        if window.len() != crate::memory::TRANSFER_BYTES {
            return Err(Refusal::Bounds);
        }
        if matches!(r, Request::Init) {
            if self.engine.is_some() {
                return Err(Refusal::Order);
            }
            self.engine = Some(Box::new(Engine::new(Initialize::decode(window)?)?));
            return Ok(Response::Ready);
        }
        let e = self.engine.as_mut().ok_or(Refusal::Order)?;
        match r {
            Request::Init => Err(Refusal::Order),
            Request::Poll { millis, ingress } => {
                if millis < e.millis || millis > i64::MAX as u64 {
                    return Err(Refusal::Clock);
                }
                if ingress as usize > FRAME_BYTES {
                    return Err(Refusal::Bounds);
                }
                if !e.adapter.empty() {
                    return Err(Refusal::Order);
                }
                let polls = e.polls.checked_add(1).ok_or(Refusal::Capacity)?;
                e.adapter.supply(&window[..ingress as usize])?;
                e.millis = millis;
                e.polls = polls;
                let now = Instant::from_millis(millis as i64);
                e.interface.poll_maintenance(now);
                e.interface
                    .poll_ingress_single(now, &mut e.adapter, &mut e.sockets);
                e.interface.poll_egress(now, &mut e.adapter, &mut e.sockets);
                Ok(e.status())
            }
            Request::TakeFrame { frame } => {
                let n = e.adapter.take(frame, window)?;
                Ok(Response::Frame {
                    frame,
                    bytes: n as u32,
                })
            }
            Request::AckFrame { frame } => {
                e.adapter.ack(frame)?;
                Ok(Response::Ready)
            }
            Request::Status => Ok(e.status()),
            Request::Write { bytes } => {
                if bytes == 0 || bytes as usize > window.len() {
                    return Err(Refusal::Bounds);
                }
                let s = e.sockets.get_mut::<tcp::Socket>(e.handle);
                if !s.may_send() || !s.is_active() {
                    return Err(Refusal::Closed);
                }
                if !s.can_send() {
                    return Err(Refusal::Order);
                }
                let n = s
                    .send_slice(&window[..bytes as usize])
                    .map_err(|_| Refusal::Send)?;
                Ok(Response::Written { bytes: n as u32 })
            }
            Request::Read { maximum } => {
                if maximum == 0 || maximum as usize > window.len() {
                    return Err(Refusal::Bounds);
                }
                let s = e.sockets.get_mut::<tcp::Socket>(e.handle);
                let n = if s.can_recv() {
                    s.recv_slice(&mut window[..maximum as usize])
                        .map_err(|_| Refusal::Receive)?
                } else if !s.may_recv() || !s.is_active() {
                    0
                } else {
                    return Err(Refusal::Order);
                };
                Ok(Response::Read { bytes: n as u32 })
            }
            Request::Close => {
                e.sockets.get_mut::<tcp::Socket>(e.handle).close();
                Ok(Response::Ready)
            }
            Request::Abort => {
                e.sockets.get_mut::<tcp::Socket>(e.handle).abort();
                Ok(Response::Ready)
            }
        }
    }
}
