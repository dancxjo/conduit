//! Actual TCP over finite memory Ethernet, with no host sockets or device I/O.
mod support;
use conduit_zenoh_pico_embassy::{BridgeAccess, SocketOwner};
use conduit_zenoh_pico_platform::{Endpoint, Fault, Phase, Progress, Refusal, SocketBridge};
use embassy_net::{
    tcp::TcpSocket, Config, Ipv4Address, Ipv4Cidr, StackResources, StaticConfigV4, TryError,
};
use std::{
    cell::RefCell,
    future::Future,
    pin::{pin, Pin},
    task::{Context, Waker},
    time::{Duration, Instant},
};
struct Access(RefCell<SocketBridge<13, 17>>);
impl BridgeAccess<13, 17> for Access {
    fn with<R>(&self, f: impl FnOnce(&mut SocketBridge<13, 17>) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }
}
fn config(last: u8) -> Config {
    Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 0, 2, last), 24),
        gateway: None,
        dns_servers: Default::default(),
    })
}
fn pump(a: Pin<&mut impl Future>, b: Pin<&mut impl Future>) {
    let mut cx = Context::from_waker(Waker::noop());
    assert!(a.poll(&mut cx).is_pending());
    assert!(b.poll(&mut cx).is_pending());
}
fn binary() -> [u8; 512] {
    std::array::from_fn(|i| (i.wrapping_mul(131) % 256) as u8)
}
#[test]
fn real_tcp_pressure_binary_close_reuse_and_unanswered_connect_deadline() {
    let (da, db) = support::pair();
    let monitor = da.clone();
    let mut ra = StackResources::<2>::new();
    let mut rb = StackResources::<2>::new();
    let (sa, mut runner_a) = embassy_net::new(da, config(1), &mut ra, 17);
    let (sb, mut runner_b) = embassy_net::new(db, config(2), &mut rb, 29);
    let mut fa = pin!(runner_a.run());
    let mut fb = pin!(runner_b.run());
    let mut arx = [0; 32];
    let mut atx = [0; 32];
    let mut brx = [0; 32];
    let mut btx = [0; 32];
    let mut client = TcpSocket::new(sa, &mut arx, &mut atx);
    let mut peer = TcpSocket::new(sb, &mut brx, &mut btx);
    let access = Access(RefCell::new(SocketBridge::new()));
    let mut owner = SocketOwner::<7>::new();
    let start = Instant::now();
    let now = || start.elapsed().as_millis() as u64;
    let target = Endpoint {
        ipv4: [192, 0, 2, 2],
        port: 7447,
    };
    assert!(matches!(peer.try_accept(7447), Err(TryError::WouldBlock)));
    let first = access.with(|b| b.request_connect(target, now() + 2000).unwrap());
    for _ in 0..2000 {
        owner.poll(&mut client, &access, now());
        pump(fa.as_mut(), fb.as_mut());
        if access.with(|b| b.phase()) == Phase::Open {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(access.with(|b| b.phase()), Phase::Open);
    let (g, local, remote) = owner.endpoints().expect("actual established endpoints");
    assert_eq!(g, first);
    assert_ne!(local.port, 0);
    assert_eq!(remote.port, 7447);
    let payload = binary();
    assert!(payload.contains(&0) && payload.contains(&255) && payload.contains(&128));
    let mut offered = 0;
    let mut pressure = false;
    // Stop the receiver: real TCP window and socket buffers must exert pressure.
    for _ in 0..100 {
        match access.with(|b| b.write(first, &payload[offered..])) {
            Ok(Progress::Bytes(n)) => offered += n,
            Ok(Progress::Pending) => pressure = true,
            other => panic!("write {other:?}"),
        }
        owner.poll(&mut client, &access, now());
        pump(fa.as_mut(), fb.as_mut());
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        pressure && offered < payload.len(),
        "finite real TCP buffers must backpressure producer"
    );
    let mut received = Vec::new();
    let mut scratch = [0; 5];
    let deadline = now() + 5000;
    while received.len() < payload.len() && now() < deadline {
        if offered < payload.len() {
            if let Ok(Progress::Bytes(n)) = access.with(|b| b.write(first, &payload[offered..])) {
                offered += n;
            }
        }
        owner.poll(&mut client, &access, now());
        pump(fa.as_mut(), fb.as_mut());
        match peer.try_read(&mut scratch) {
            Ok(n) => received.extend_from_slice(&scratch[..n]),
            Err(TryError::WouldBlock) => (),
            other => panic!("peer read {other:?}"),
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(received, payload);
    // Reverse direction uses the owner's real socket read and tiny RX ring.
    let mut sent = 0;
    let mut returned = Vec::new();
    let deadline = now() + 5000;
    while returned.len() < payload.len() && now() < deadline {
        if sent < payload.len() {
            match peer.try_write(&payload[sent..]) {
                Ok(n) => sent += n,
                Err(TryError::WouldBlock) => (),
                other => panic!("peer write {other:?}"),
            }
        }
        pump(fa.as_mut(), fb.as_mut());
        owner.poll(&mut client, &access, now());
        let mut small = [0; 3];
        match access.with(|b| b.read(first, &mut small)) {
            Ok(Progress::Bytes(n)) => returned.extend_from_slice(&small[..n]),
            Ok(Progress::Pending) => (),
            other => panic!("bridge read {other:?}"),
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(returned, payload);
    access.with(|b| b.request_close(first).unwrap());
    owner.poll(&mut client, &access, now());
    assert_eq!(access.with(|b| b.phase()), Phase::Idle);
    assert!(owner.endpoints().is_none());
    peer.abort();
    pump(fa.as_mut(), fb.as_mut());
    assert!(matches!(peer.try_accept(7447), Err(TryError::WouldBlock)));
    let second = access.with(|b| b.request_connect(target, now() + 2000).unwrap());
    assert_ne!(first, second);
    for _ in 0..2000 {
        owner.poll(&mut client, &access, now());
        pump(fa.as_mut(), fb.as_mut());
        if access.with(|b| b.phase()) == Phase::Open {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(access.with(|b| b.phase()), Phase::Open);
    assert_eq!(
        access.with(|b| b.network_receive(first, &[99])),
        Err(Refusal::Stale)
    );
    assert_eq!(
        access.with(|b| b.read(second, &mut scratch)),
        Ok(Progress::Pending)
    );
    // A real FIN must not discard the accepted prefix buffered by the owner.
    let tail = [0, 255, 128, 3, 4, 5, 6, 7];
    assert_eq!(peer.try_write(&tail), Ok(tail.len()));
    peer.close();
    let deadline = now() + 2000;
    while access.with(|b| b.phase()) == Phase::Open && now() < deadline {
        pump(fa.as_mut(), fb.as_mut());
        owner.poll(&mut client, &access, now());
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(access.with(|b| b.phase()), Phase::PeerClosed);
    let mut drained = [0; 13];
    assert_eq!(
        access.with(|b| b.read(second, &mut drained)),
        Ok(Progress::Bytes(tail.len()))
    );
    assert_eq!(&drained[..tail.len()], &tail);
    assert_eq!(
        access.with(|b| b.read(second, &mut drained)),
        Ok(Progress::Eof)
    );
    access.with(|b| b.request_close(second).unwrap());
    owner.poll(&mut client, &access, now());
    // No machine answers ARP at .99: deadline must abort the same real SYN attempt.
    let third = access.with(|b| {
        b.request_connect(
            Endpoint {
                ipv4: [192, 0, 2, 99],
                port: 7447,
            },
            now() + 20,
        )
        .unwrap()
    });
    let deadline = now() + 1000;
    while access.with(|b| b.phase()) == Phase::Connecting && now() < deadline {
        owner.poll(&mut client, &access, now());
        pump(fa.as_mut(), fb.as_mut());
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(access.with(|b| b.phase()), Phase::Failed(Fault::TimedOut));
    assert!(owner.endpoints().is_none());
    access.with(|b| b.request_close(third).unwrap());
    owner.poll(&mut client, &access, now());
    assert_eq!(access.with(|b| b.phase()), Phase::Idle);
    // Cancellation before a handshake must retire the actual attempt too.
    let fourth = access.with(|b| {
        b.request_connect(
            Endpoint {
                ipv4: [192, 0, 2, 99],
                port: 7447,
            },
            now() + 2000,
        )
        .unwrap()
    });
    owner.poll(&mut client, &access, now());
    access.with(|b| b.request_close(fourth).unwrap());
    owner.poll(&mut client, &access, now());
    assert_eq!(access.with(|b| b.phase()), Phase::Idle);
    assert_eq!(client.state(), embassy_net::tcp::State::Closed);
    // A reachable peer with no listener refuses once; it must not be retried
    // silently until the preparation deadline by repeated try_connect calls.
    let fifth = access.with(|b| {
        b.request_connect(
            Endpoint {
                ipv4: [192, 0, 2, 2],
                port: 7448,
            },
            now() + 2000,
        )
        .unwrap()
    });
    let deadline = now() + 1000;
    while access.with(|b| b.phase()) == Phase::Connecting && now() < deadline {
        owner.poll(&mut client, &access, now());
        pump(fa.as_mut(), fb.as_mut());
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(access.with(|b| b.phase()), Phase::Failed(Fault::Network));
    access.with(|b| b.request_close(fifth).unwrap());
    owner.poll(&mut client, &access, now());
    let (frames, high) = monitor.counters();
    assert!(frames > 10);
    assert!(high <= 2);
}
