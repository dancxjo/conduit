//! Fixed-frame simulation of the real smoltcp session, below native/USB truth.
#![cfg(feature = "native-owner-network")]
use conduitos::virtio_tcp::{TcpExchange, TcpExchangeOptions, VirtioTcpEndpoint, VirtioTcpError};
use smoltcp::{
    iface::{Config, Interface, SocketSet, SocketStorage},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    socket::tcp,
    time::Instant,
    wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
};

thread_local! { static MAXIMUM_WIRE_BYTES: Cell<usize> = const { Cell::new(0) }; }

struct CountingAllocator;
thread_local! { static COUNTING: Cell<bool> = const { Cell::new(false) }; static ALLOCATIONS: Cell<usize> = const { Cell::new(0) }; }
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNTING.with(|active| {
            if active.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const FRAME: usize = 1514;
const SLOTS: usize = 4;
struct Frames {
    wire: [[u8; FRAME]; SLOTS],
    length: [usize; SLOTS],
    head: usize,
    items: usize,
}
impl Frames {
    fn new() -> RefCell<Self> {
        RefCell::new(Self {
            wire: [[0; FRAME]; SLOTS],
            length: [0; SLOTS],
            head: 0,
            items: 0,
        })
    }
}
struct FixedDevice<'a> {
    incoming: &'a RefCell<Frames>,
    outgoing: &'a RefCell<Frames>,
    receives: usize,
}
struct Receive {
    wire: [u8; FRAME],
    length: usize,
}
impl RxToken for Receive {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(&self.wire[..self.length])
    }
}
struct Transmit<'a>(&'a RefCell<Frames>);
impl TxToken for Transmit<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, length: usize, f: F) -> R {
        assert!(length <= FRAME);
        let mut frames = self.0.borrow_mut();
        assert!(frames.items < SLOTS);
        let tail = (frames.head + frames.items) % SLOTS;
        let result = f(&mut frames.wire[tail][..length]);
        MAXIMUM_WIRE_BYTES.with(|maximum| maximum.set(maximum.get().max(length)));
        frames.length[tail] = length;
        frames.items += 1;
        result
    }
}
impl Device for FixedDevice<'_> {
    type RxToken<'a>
        = Receive
    where
        Self: 'a;
    type TxToken<'a>
        = Transmit<'a>
    where
        Self: 'a;
    fn receive(&mut self, _: Instant) -> Option<(Receive, Transmit<'_>)> {
        self.receives += 1;
        if self.outgoing.borrow().items == SLOTS {
            return None;
        }
        let mut frames = self.incoming.borrow_mut();
        if frames.items == 0 {
            return None;
        }
        let head = frames.head;
        let token = Receive {
            wire: frames.wire[head],
            length: frames.length[head],
        };
        frames.head = (head + 1) % SLOTS;
        frames.items -= 1;
        Some((token, Transmit(self.outgoing)))
    }
    fn transmit(&mut self, _: Instant) -> Option<Transmit<'_>> {
        (self.outgoing.borrow().items < SLOTS).then_some(Transmit(self.outgoing))
    }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = FRAME;
        caps
    }
}
fn options(maximum_polls: u32) -> TcpExchangeOptions {
    TcpExchangeOptions {
        random_seed: 1,
        mac: [2, 0, 0, 0, 0, 1],
        timestamp: Instant::ZERO,
        maximum_polls,
        endpoint: VirtioTcpEndpoint {
            guest_address: [10, 0, 0, 1],
            prefix_length: 24,
            gateway: [10, 0, 0, 2],
            remote_address: [10, 0, 0, 2],
            remote_port: 9000,
            local_port: 49152,
        },
    }
}

#[test]
fn retained_session_interleaves_real_peer_frames_and_allocates_nothing_during_poll() {
    exchange_request(b"ping");
}

#[test]
fn full_mtu_payload_uses_admitted_storage_without_poll_allocations() {
    exchange_request(&[0x5a; 1460]);
}

fn exchange_request(request: &[u8]) {
    let client_in = Frames::new();
    let peer_in = Frames::new();
    let mut client = FixedDevice {
        incoming: &client_in,
        outgoing: &peer_in,
        receives: 0,
    };
    let mut peer = FixedDevice {
        incoming: &peer_in,
        outgoing: &client_in,
        receives: 0,
    };
    let mut receive = [0; 2048];
    let mut transmit = [0; 2048];
    let mut storage = [SocketStorage::EMPTY];
    let mut response = [0; 4];
    let mut exchange = TcpExchange::prepare(
        &mut client,
        options(1000),
        request,
        &mut response,
        4,
        &mut receive,
        &mut transmit,
        &mut storage,
    )
    .unwrap();
    let config = Config::new(HardwareAddress::Ethernet(EthernetAddress([
        2, 0, 0, 0, 0, 2,
    ])));
    let mut interface = Interface::new(config, &mut peer, Instant::ZERO);
    interface.update_ip_addrs(|addresses| {
        addresses
            .push(IpCidr::new(
                IpAddress::Ipv4(Ipv4Address::new(10, 0, 0, 2)),
                24,
            ))
            .unwrap();
    });
    let mut peer_rx = [0; 2048];
    let mut peer_tx = [0; 2048];
    let mut peer_storage = [SocketStorage::EMPTY];
    let mut socket = tcp::Socket::new(
        tcp::SocketBuffer::new(&mut peer_rx[..]),
        tcp::SocketBuffer::new(&mut peer_tx[..]),
    );
    socket.listen(9000).unwrap();
    let mut sockets = SocketSet::new(&mut peer_storage[..]);
    let handle = sockets.add(socket);
    let mut observed = [0; 2048];
    let mut received = 0;
    let mut replied = false;
    let mut receipt = None;
    MAXIMUM_WIRE_BYTES.with(|maximum| maximum.set(0));
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|active| active.set(true));
    for poll in 1..=1000 {
        let now = Instant::from_millis(poll);
        let previous = client.receives;
        let result = exchange.poll_step(&mut client, now).unwrap();
        assert!(client.receives - previous <= 1);
        if result.is_some() {
            receipt = result;
            break;
        }
        // Deliberately leave the peer dormant; queue pressure cannot create a
        // private drain loop in the client's cooperative step.
        if poll < 50 {
            continue;
        }
        interface.poll_maintenance(now);
        interface.poll_ingress_single(now, &mut peer, &mut sockets);
        interface.poll_egress(now, &mut peer, &mut sockets);
        let socket = sockets.get_mut::<tcp::Socket>(handle);
        if socket.can_recv() && received < request.len() {
            received += socket
                .recv_slice(&mut observed[received..request.len()])
                .unwrap();
        }
        if received == request.len() && !replied && socket.can_send() {
            assert_eq!(socket.send_slice(b"pong").unwrap(), 4);
            socket.close();
            replied = true;
        }
    }
    COUNTING.with(|active| active.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let receipt = receipt.expect("bounded actual TCP handshake, data and FIN acknowledgment");
    assert_eq!(usize::from(receipt.transmitted_bytes), request.len());
    assert_eq!(receipt.received_bytes, 4);
    assert_eq!(exchange.received(), b"pong");
    assert_eq!(&observed[..received], request);
    if request.len() == 1460 {
        assert_eq!(MAXIMUM_WIRE_BYTES.with(Cell::get), FRAME);
    }
    let previous = client.receives;
    assert_eq!(
        exchange.poll_step(&mut client, Instant::from_millis(1001)),
        Ok(Some(receipt))
    );
    exchange.cancel();
    assert_eq!(
        exchange.poll_step(&mut client, Instant::from_millis(1002)),
        Ok(Some(receipt))
    );
    assert_eq!(client.receives, previous);
}

#[test]
fn timeout_cancellation_and_clock_regression_are_sticky_without_further_device_calls() {
    for outcome in [
        VirtioTcpError::Timeout,
        VirtioTcpError::Cancelled,
        VirtioTcpError::ClockRegressed,
    ] {
        let incoming = Frames::new();
        let outgoing = Frames::new();
        let mut device = FixedDevice {
            incoming: &incoming,
            outgoing: &outgoing,
            receives: 0,
        };
        let mut rx = [0; 1024];
        let mut tx = [0; 1024];
        let mut storage = [SocketStorage::EMPTY];
        let mut response = [0; 4];
        let mut exchange = TcpExchange::prepare(
            &mut device,
            options(2),
            b"ping",
            &mut response,
            4,
            &mut rx,
            &mut tx,
            &mut storage,
        )
        .unwrap();
        assert_eq!(
            exchange.poll_step(&mut device, Instant::from_millis(1)),
            Ok(None)
        );
        let result = match outcome {
            VirtioTcpError::Cancelled => {
                exchange.cancel();
                exchange.poll_step(&mut device, Instant::from_millis(2))
            }
            VirtioTcpError::ClockRegressed => exchange.poll_step(&mut device, Instant::ZERO),
            _ => exchange.poll_step(&mut device, Instant::from_millis(2)),
        };
        assert_eq!(result, Err(outcome));
        let previous = device.receives;
        assert_eq!(
            exchange.poll_step(&mut device, Instant::from_millis(3)),
            Err(outcome)
        );
        assert_eq!(device.receives, previous);
    }
}

#[test]
fn preparation_refuses_bounds_before_touching_device_queues() {
    let incoming = Frames::new();
    let outgoing = Frames::new();
    let mut device = FixedDevice {
        incoming: &incoming,
        outgoing: &outgoing,
        receives: 0,
    };
    let mut rx = [0; 1024];
    let mut tx = [0; 1024];
    let mut storage = [SocketStorage::EMPTY];
    let mut response = [0; 4];
    assert!(matches!(
        TcpExchange::prepare(
            &mut device,
            options(0),
            b"ping",
            &mut response,
            4,
            &mut rx,
            &mut tx,
            &mut storage
        ),
        Err(VirtioTcpError::InvalidEndpoint)
    ));
    assert_eq!(device.receives, 0);
    assert_eq!(incoming.borrow().items, 0);
    assert_eq!(outgoing.borrow().items, 0);
}

#[test]
fn full_frame_queue_preserves_ingress_and_returns_after_one_receive_attempt() {
    let incoming = Frames::new();
    let outgoing = Frames::new();
    incoming.borrow_mut().items = SLOTS;
    outgoing.borrow_mut().items = SLOTS;
    let mut device = FixedDevice {
        incoming: &incoming,
        outgoing: &outgoing,
        receives: 0,
    };
    let mut rx = [0; 1024];
    let mut tx = [0; 1024];
    let mut storage = [SocketStorage::EMPTY];
    let mut response = [0; 4];
    let mut exchange = TcpExchange::prepare(
        &mut device,
        options(2),
        b"ping",
        &mut response,
        4,
        &mut rx,
        &mut tx,
        &mut storage,
    )
    .unwrap();
    assert_eq!(
        exchange.poll_step(&mut device, Instant::from_millis(1)),
        Ok(None)
    );
    assert_eq!(device.receives, 1);
    assert_eq!(incoming.borrow().items, SLOTS);
    assert_eq!(outgoing.borrow().items, SLOTS);
    assert_eq!(
        exchange.poll_step(&mut device, Instant::from_millis(2)),
        Err(VirtioTcpError::Timeout)
    );
    assert_eq!(device.receives, 2);
    assert_eq!(incoming.borrow().items, SLOTS);
}

#[test]
fn preparation_preserves_storage_and_receipt_bounds_before_device_effects() {
    for (request_bytes, response_bytes, transmit_bytes, receive_bytes, expected) in [
        (5, 4, 4, 4, VirtioTcpError::RequestTooLarge),
        (4, 5, 4, 4, VirtioTcpError::ResponseTooLarge),
        (65536, 4, 65536, 4, VirtioTcpError::RequestTooLarge),
        (4, 65536, 4, 65536, VirtioTcpError::ResponseTooLarge),
    ] {
        let incoming = Frames::new();
        let outgoing = Frames::new();
        let mut device = FixedDevice {
            incoming: &incoming,
            outgoing: &outgoing,
            receives: 0,
        };
        let request = vec![0x5a; request_bytes];
        let mut response = vec![0; response_bytes];
        let mut receive = vec![0; receive_bytes];
        let mut transmit = vec![0; transmit_bytes];
        let mut storage = [SocketStorage::EMPTY];
        let result = TcpExchange::prepare(
            &mut device,
            options(1000),
            &request,
            &mut response,
            response_bytes,
            &mut receive,
            &mut transmit,
            &mut storage,
        );
        assert!(matches!(result, Err(error) if error == expected));
        assert_eq!(device.receives, 0);
        assert_eq!(incoming.borrow().items, 0);
        assert_eq!(outgoing.borrow().items, 0);
    }
}
