use super::*;
use conduit_core::{bind_active_play, InfoBool};
use conduit_kernel::RequestId;
use std::{
    os::{fd::OwnedFd, unix::net::UnixStream},
    thread::JoinHandle,
};

// Exercise the actual no-allocation firmware decoder against the native wire
// provider. This shared-source conformance is not USB or physical LED proof.
#[path = "../../../rp2040/firmware/pico-w-signal/src/indicator_resource/protocol.rs"]
mod peripheral;

fn pair() -> (Port, UnixStream) {
    let (client, server) = UnixStream::pair().unwrap();
    client.set_nonblocking(true).unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    server
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let fd: OwnedFd = client.into();
    (Port(File::from(fd)), server)
}

fn provider(mutation: Option<usize>, count: usize) -> (PicoIndicator, JoinHandle<()>) {
    let (client, mut server) = pair();
    let task = std::thread::spawn(move || {
        let mut session = peripheral::Session::new([2; 16], [3; 32]);
        for index in 0..=count {
            let mut frame = [0; BYTES];
            server.read_exact(&mut frame).unwrap();
            let mut response = match session.accept(frame).unwrap() {
                peripheral::Command::Ready(response) => response,
                peripheral::Command::Set {
                    state,
                    acknowledgment,
                } => {
                    assert_eq!(state, index % 2 == 1);
                    acknowledgment
                }
            };
            if index != 0 {
                if let Some(byte) = mutation {
                    response[byte] ^= 1;
                }
            }
            // Deliberate fragmentation tests the byte-stream/USB-packet boundary.
            for chunk in response.chunks(7) {
                server.write_all(chunk).unwrap();
            }
        }
    });
    let host = crate::StdHost::new().advertisement().clone();
    let adapter =
        PicoIndicator::acquire_port(client, &host, [3; 32], [1; 16], Duration::from_secs(2))
            .unwrap();
    (adapter, task)
}

#[test]
fn native_provider_and_firmware_correlate_all_eight_acknowledgments() {
    let (mut provider, task) = provider(None, 8);
    let play = bind_active_play(
        &"proof/plan".into(),
        &provider.binding.host_id,
        &provider.binding.boot_id,
        0,
    );
    for request in 0..8 {
        provider
            .present(IndicatorRequest {
                play: &play,
                request: RequestId(request),
                state: InfoBool::new(request % 2 == 0),
            })
            .unwrap();
    }
    assert_eq!(provider.receipts().len(), 8);
    assert_eq!(provider.device_boot(), &[2; 16]);
    assert_eq!(provider.firmware_digest(), &[3; 32]);
    assert!(provider
        .binding
        .pool_id
        .as_str()
        .ends_with(&"01".repeat(16)));
    task.join().unwrap();
}

#[test]
fn bad_acknowledgments_poison_provider_without_retry() {
    for (byte, expected) in [
        (0, IndicatorFailure::MalformedReceipt),
        (4, IndicatorFailure::MalformedReceipt),
        (5, IndicatorFailure::WrongState),
        (8, IndicatorFailure::StaleIdentity),
        (24, IndicatorFailure::StaleIdentity),
        (40, IndicatorFailure::StaleIdentity),
        (72, IndicatorFailure::StaleIdentity),
        (80, IndicatorFailure::StaleIdentity),
    ] {
        let (mut provider, task) = provider(Some(byte), 1);
        let play = bind_active_play(
            &"proof/plan".into(),
            &provider.binding.host_id,
            &provider.binding.boot_id,
            0,
        );
        for _ in 0..2 {
            assert_eq!(
                provider.present(IndicatorRequest {
                    play: &play,
                    request: RequestId(0),
                    state: InfoBool::new(true)
                }),
                Err(expected)
            );
        }
        assert!(provider.receipts().is_empty());
        assert!(matches!(
            provider.device_association().disposition,
            conduit_core::DeviceTruthDisposition::HistoricalLost { .. }
        ));
        task.join().unwrap();
    }
}

#[test]
fn optional_device_provenance_is_boot_local_and_does_not_create_capabilities() {
    let (provider, task) = provider(None, 0);
    let device = provider.device_association();
    let mut host = crate::StdHost::new().advertisement().clone();
    host.host_id = provider.binding.host_id.clone();
    host.boot_id = provider.binding.boot_id.clone();
    host.offer_generation = provider.binding.offer_generation;
    host.capabilities = vec![conduit_std_offers::indicator_resource::offer()];
    conduit_core::validate_device_associations(&host, std::slice::from_ref(&device)).unwrap();
    assert_eq!(
        device.identity_evidence.strength,
        conduit_core::DeviceIdentityStrength::BootLocalResource
    );
    assert!(!device
        .identity_evidence
        .facts
        .iter()
        .any(|fact| fact.name == "physical-serial"));
    host.offer_generation.0 += 1;
    assert_eq!(
        conduit_core::validate_device_associations(&host, &[device]),
        Err(conduit_core::DeviceAssociationRefusal::WrongCurrentGeneration)
    );
    task.join().unwrap();
}

#[test]
fn missing_and_old_firmware_fail_finitely() {
    let host = crate::StdHost::new().advertisement().clone();
    let (client, _silent_server) = pair();
    assert_eq!(
        PicoIndicator::acquire_port(client, &host, [3; 32], [1; 16], Duration::from_millis(20))
            .err(),
        Some(IndicatorFailure::Timeout)
    );
    let (client, server) = pair();
    drop(server);
    assert_eq!(
        PicoIndicator::acquire_port(client, &host, [3; 32], [1; 16], Duration::from_secs(1)).err(),
        Some(IndicatorFailure::Lost)
    );
    let (client, mut server) = pair();
    let task = std::thread::spawn(move || {
        let mut hello = [0; BYTES];
        server.read_exact(&mut hello).unwrap();
        let mut session = peripheral::Session::new([2; 16], [9; 32]);
        let peripheral::Command::Ready(response) = session.accept(hello).unwrap() else {
            panic!()
        };
        server.write_all(&response).unwrap();
    });
    assert_eq!(
        PicoIndicator::acquire_port(client, &host, [3; 32], [1; 16], Duration::from_secs(1)).err(),
        Some(IndicatorFailure::StaleIdentity)
    );
    task.join().unwrap();
}
