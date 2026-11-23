use super::*;
use std::net::TcpListener;

#[test]
fn one_nonblocking_connection_preserves_the_exact_ipv4_or_ipv6_peer_without_allocating() {
    for address in ["127.0.0.1:0", "[::1]:0"] {
        let listener = match TcpListener::bind(address) {
            Ok(listener) => listener,
            Err(error)
                if address.starts_with('[')
                    && matches!(
                        error.kind(),
                        io::ErrorKind::AddrNotAvailable | io::ErrorKind::Unsupported
                    ) =>
            {
                continue
            }
            Err(error) => panic!("loopback fixture: {error}"),
        };
        let endpoint = listener.local_addr().unwrap();
        let probe = crate::allocation_probe::begin();
        let stream = connect(
            endpoint,
            Instant::now() + std::time::Duration::from_secs(2),
            &|| false,
        )
        .unwrap();
        assert_eq!(stream.peer_addr().unwrap(), endpoint);
        // SAFETY: read-only descriptor flag queries on this live owned socket.
        assert_ne!(
            unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
        assert_ne!(
            unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        drop(stream);
        assert_eq!(probe.finish_bytes(), 0);
    }
}
#[test]
fn refused_cancelled_and_expired_connections_allocate_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    drop(listener);
    let probe = crate::allocation_probe::begin();
    assert!(matches!(
        connect(
            endpoint,
            Instant::now() + std::time::Duration::from_secs(2),
            &|| false
        ),
        Err(Error::Connect)
    ));
    assert!(matches!(
        connect(
            endpoint,
            Instant::now() + std::time::Duration::from_secs(2),
            &|| true
        ),
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        connect(endpoint, Instant::now(), &|| false),
        Err(Error::Deadline)
    ));
    assert_eq!(probe.finish_bytes(), 0);
}

// Linux's zero listen backlog admits one queued handshake. Keeping that slot
// occupied makes the next actual loopback connect wait without external routing,
// firewall changes, packet injection or repeated provider connection attempts.
#[cfg(target_os = "linux")]
fn full_backlog() -> (TcpListener, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    // SAFETY: listener owns a live TCP listening descriptor; change only its
    // private fixture backlog, never a shared/production socket.
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
    let queued = TcpStream::connect_timeout(
        &listener.local_addr().unwrap(),
        std::time::Duration::from_secs(1),
    )
    .unwrap();
    (listener, queued)
}
#[cfg(target_os = "linux")]
#[test]
fn pending_connect_uses_the_original_total_deadline_and_checks_cancellation_without_output() {
    use super::super::{OpenMeteoHttp, MAXIMUM_WAIT};
    use std::time::Duration;
    let (listener, _queued) = full_backlog();
    let mut provider = OpenMeteoHttp::prepare(listener.local_addr().unwrap()).unwrap();
    let mut output = [0xA5; 32];
    let tick = conduit_time::encode_tick(0);
    let began = Instant::now();
    let cancel_at = began + Duration::from_millis(5);
    let probe = crate::allocation_probe::begin();
    let result = provider.read_into(&tick, &mut output, began + MAXIMUM_WAIT, || {
        Instant::now() >= cancel_at
    });
    assert_eq!(probe.finish_bytes(), 0);
    assert_eq!(result, Err(Error::Cancelled));
    assert!(output.iter().all(|b| *b == 0xA5));
    assert!(
        began.elapsed() < Duration::from_millis(500),
        "cooperative connect must not consume the worker's500ms cancellation budget"
    );
    let began = Instant::now();
    let probe = crate::allocation_probe::begin();
    let result = provider.read_into(
        &tick,
        &mut output,
        began + Duration::from_millis(30),
        || false,
    );
    assert_eq!(probe.finish_bytes(), 0);
    assert_eq!(result, Err(Error::Deadline));
    assert!(output.iter().all(|b| *b == 0xA5));
    let began = Instant::now();
    let result = provider.read_into(&tick, &mut output, began + Duration::from_secs(60), || {
        false
    });
    assert_eq!(result, Err(Error::Deadline));
    assert!(began.elapsed() < MAXIMUM_WAIT + Duration::from_millis(500));
    assert!(output.iter().all(|b| *b == 0xA5));
}
#[cfg(target_os = "linux")]
#[test]
fn a_single_connection_may_complete_after_the_old250ms_slice_within_the_same_two_seconds() {
    use std::{thread, time::Duration};
    let (listener, _queued) = full_backlog();
    let endpoint = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        drop(listener.accept().unwrap());
        let mut readable = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one live private listener, finite fixture-only acceptance wait.
        assert_eq!(unsafe { libc::poll(&mut readable, 1, 2000) }, 1);
        drop(listener.accept().unwrap());
    });
    let began = Instant::now();
    let probe = crate::allocation_probe::begin();
    let result = connect(endpoint, began + Duration::from_secs(2), &|| false);
    assert_eq!(probe.finish_bytes(), 0);
    assert!(
        result.is_ok(),
        "one pending connection should complete after backlog pressure releases: {result:?}"
    );
    assert!(began.elapsed() >= Duration::from_millis(250));
    server.join().unwrap();
}
