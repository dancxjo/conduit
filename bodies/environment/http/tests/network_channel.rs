//! Actual asynchronous mailbox ownership; no radio or authority fixture.
use conduit_environment_http::network_setup::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
fn poll<T>(future: Pin<&mut impl Future<Output = T>>) -> Poll<T> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}
fn input() -> NetworkSetupInput {
    NetworkSetupInput::from_form(
        b"ssid=test&password=private-password",
        NetworkSetupLimits {
            minimum_credential_bytes: 8,
            maximum_credential_bytes: 63,
        },
    )
    .unwrap()
}
#[test]
fn socket_custody_and_flush_precede_selected_read_and_capacity_preserves_input() {
    let mut channel = NetworkSetupChannel::new(1);
    let (writer, mut reader) = channel.split();
    let second_writer = writer.clone();
    assert_eq!(reader.has_acknowledged(), Ok(false));
    let mut lock = std::pin::pin!(writer.lock());
    let Poll::Ready(mut guard) = poll(lock.as_mut()) else {
        panic!("uncontended mutex");
    };
    let mut waiting = std::pin::pin!(second_writer.lock());
    assert!(poll(waiting.as_mut()).is_pending());
    assert_eq!(
        reader.has_acknowledged(),
        Err(NetworkSetupReadRefusal::Busy)
    );
    let mut output = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    assert_eq!(
        reader.take_acknowledged(&mut output),
        Err(NetworkSetupReadRefusal::Busy)
    );
    let pending = PendingNetworkResponse::prepare(&mut guard, input()).unwrap();
    pending.response_flushed().unwrap();
    assert_eq!(
        reader.has_acknowledged(),
        Err(NetworkSetupReadRefusal::Busy)
    );
    drop(guard);
    assert_eq!(reader.has_acknowledged(), Ok(true));
    assert!(matches!(
        reader.take_acknowledged(&mut []),
        Err(NetworkSetupReadRefusal::Input(NetworkSetupRefusal::Wire(_)))
    ));
    assert_eq!(reader.has_acknowledged(), Ok(true));
    let count = reader.take_acknowledged(&mut output).unwrap();
    let request = conduit_net::decode_network_join_request(&output[..count]).unwrap();
    assert_eq!(request.ssid, b"test");
    assert_eq!(request.credential, b"private-password");
    assert_eq!(
        reader.take_acknowledged(&mut output),
        Err(NetworkSetupReadRefusal::Input(
            NetworkSetupRefusal::NoSubmission
        ))
    );
    let Poll::Ready(mut guard) = poll(waiting.as_mut()) else {
        panic!("released mutex");
    };
    assert!(matches!(
        PendingNetworkResponse::prepare(&mut guard, input()),
        Err(NetworkSetupRefusal::Exhausted)
    ));
}
#[test]
fn cancelled_response_cannot_leave_acknowledged_input_for_the_selected_source() {
    let mut channel = NetworkSetupChannel::new(1);
    let (writer, mut reader) = channel.split();
    let mut lock = std::pin::pin!(writer.lock());
    let Poll::Ready(mut guard) = poll(lock.as_mut()) else {
        panic!("uncontended mutex");
    };
    let pending = PendingNetworkResponse::prepare(&mut guard, input()).unwrap();
    drop(pending);
    drop(guard);
    assert_eq!(reader.has_acknowledged(), Ok(false));
    assert_eq!(
        reader.take_acknowledged(&mut [0; 167]),
        Err(NetworkSetupReadRefusal::Input(
            NetworkSetupRefusal::NoSubmission
        ))
    );
}

#[test]
fn root_preparation_requires_flush_and_preserves_the_exact_selected_input() {
    let mut channel = NetworkSetupChannel::new(1);
    let (writer, mut reader) = channel.split();
    assert_eq!(
        reader.prepare_acknowledged(|_| panic!("no input")),
        Err(NetworkSetupReadRefusal::Input(
            NetworkSetupRefusal::NoSubmission
        ))
    );
    let mut lock = std::pin::pin!(writer.lock());
    let Poll::Ready(mut guard) = poll(lock.as_mut()) else {
        panic!("uncontended mutex");
    };
    let submission = guard.submit(input()).unwrap();
    assert_eq!(
        guard.prepare_acknowledged(|_| panic!("not flushed")),
        Err(NetworkSetupRefusal::NotAcknowledged)
    );
    guard.acknowledge(submission).unwrap();
    assert_eq!(
        reader.prepare_acknowledged(|_| panic!("socket still owns input")),
        Err(NetworkSetupReadRefusal::Busy)
    );
    drop(guard);
    let prepared = reader
        .prepare_acknowledged(|request| {
            assert_eq!(request.ssid, b"test");
            assert_eq!(request.credential, b"private-password");
            // Stand-in for a fallible SDK preparation result, not a radio provider.
            Err::<(), _>("configuration refused")
        })
        .unwrap();
    assert_eq!(prepared, Err("configuration refused"));
    assert_eq!(reader.has_acknowledged(), Ok(true));
    let mut output = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    let length = reader.take_acknowledged(&mut output).unwrap();
    let request = conduit_net::decode_network_join_request(&output[..length]).unwrap();
    assert_eq!(request.ssid, b"test");
    assert_eq!(request.credential, b"private-password");
    assert_eq!(
        reader.prepare_acknowledged(|_| panic!("already consumed")),
        Err(NetworkSetupReadRefusal::Input(
            NetworkSetupRefusal::NoSubmission
        ))
    );
}
