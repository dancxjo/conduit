//! Packet driver only: TCP/ARP and socket state are supplied by real Embassy.
use embassy_net_driver::{Capabilities, Driver, HardwareAddress, LinkState, RxToken, TxToken};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    task::{Context, Waker},
};
const MTU: usize = 1514;
const DEPTH: usize = 2;
struct Packet {
    bytes: [u8; MTU],
    len: usize,
}
struct Wire {
    queues: [VecDeque<Packet>; 2],
    wake: [Option<Waker>; 2],
    frames: usize,
    high: usize,
}
#[derive(Clone)]
pub struct Port {
    wire: Rc<RefCell<Wire>>,
    side: usize,
}
pub fn pair() -> (Port, Port) {
    let wire = Rc::new(RefCell::new(Wire {
        queues: std::array::from_fn(|_| VecDeque::with_capacity(DEPTH)),
        wake: [None, None],
        frames: 0,
        high: 0,
    }));
    (
        Port {
            wire: wire.clone(),
            side: 0,
        },
        Port { wire, side: 1 },
    )
}
impl Port {
    pub fn counters(&self) -> (usize, usize) {
        let w = self.wire.borrow();
        (w.frames, w.high)
    }
    fn register(&self, cx: &Context<'_>) {
        self.wire.borrow_mut().wake[self.side] = Some(cx.waker().clone());
    }
}
pub struct Receive(Packet);
pub struct Transmit(Port);
impl RxToken for Receive {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(mut self, f: F) -> R {
        f(&mut self.0.bytes[..self.0.len])
    }
}
impl TxToken for Transmit {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        assert!(len <= MTU);
        let mut p = Packet {
            bytes: [0; MTU],
            len,
        };
        let result = f(&mut p.bytes[..len]);
        let mut w = self.0.wire.borrow_mut();
        let target = 1 - self.0.side;
        assert!(
            w.queues[target].len() < DEPTH,
            "driver violated transmit-token capacity"
        );
        w.queues[target].push_back(p);
        w.frames += 1;
        w.high = w.high.max(w.queues[target].len());
        if let Some(waker) = w.wake[target].take() {
            waker.wake();
        }
        result
    }
}
impl Driver for Port {
    type RxToken<'a> = Receive;
    type TxToken<'a> = Transmit;
    fn receive(&mut self, cx: &mut Context<'_>) -> Option<(Receive, Transmit)> {
        self.register(cx);
        let mut w = self.wire.borrow_mut();
        if w.queues[1 - self.side].len() == DEPTH {
            return None;
        }
        let packet = w.queues[self.side].pop_front()?;
        if let Some(waker) = w.wake[1 - self.side].take() {
            waker.wake();
        }
        Some((Receive(packet), Transmit(self.clone())))
    }
    fn transmit(&mut self, cx: &mut Context<'_>) -> Option<Transmit> {
        self.register(cx);
        (self.wire.borrow().queues[1 - self.side].len() < DEPTH).then(|| Transmit(self.clone()))
    }
    fn link_state(&mut self, cx: &mut Context<'_>) -> LinkState {
        self.register(cx);
        LinkState::Up
    }
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::default();
        c.max_transmission_unit = MTU;
        c.max_burst_size = Some(DEPTH);
        c
    }
    fn hardware_address(&self) -> HardwareAddress {
        HardwareAddress::Ethernet([2, 0, 0, 0, 0, self.side as u8 + 1])
    }
}
