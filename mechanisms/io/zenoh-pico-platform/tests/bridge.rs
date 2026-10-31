use conduit_zenoh_pico_platform::*;
const PEER: Endpoint = Endpoint {
    ipv4: [192, 0, 2, 1],
    port: 7447,
};
fn open<const R: usize, const T: usize>(bridge: &mut SocketBridge<R, T>) -> SocketGeneration {
    let generation = bridge.request_connect(PEER, 100).unwrap();
    assert_eq!(
        bridge.take_command(1),
        Some(NetworkCommand::Connect {
            generation,
            endpoint: PEER,
            deadline_ms: 100,
        })
    );
    assert_eq!(bridge.take_command(2), None);
    assert_eq!(
        bridge.complete_connect(generation, 2, Ok(())),
        Ok(Phase::Open)
    );
    generation
}
fn close<const R: usize, const T: usize>(
    bridge: &mut SocketBridge<R, T>,
    generation: SocketGeneration,
) {
    bridge.request_close(generation).unwrap();
    assert_eq!(bridge.request_connect(PEER, 200), Err(Refusal::Busy));
    assert_eq!(bridge.complete_close(generation), Err(Refusal::State));
    assert_eq!(
        bridge.take_command(3),
        Some(NetworkCommand::Close { generation })
    );
    assert_eq!(bridge.take_command(4), None);
    bridge.complete_close(generation).unwrap();
}
#[test]
fn transmit_pressure_partial_writes_and_wrapping_preserve_exact_binary_suffixes() {
    let mut bridge = SocketBridge::<4, 4>::new();
    let generation = open(&mut bridge);
    assert_eq!(
        bridge.write(generation, &[0, 255, 128, 3, 4]),
        Ok(Progress::Bytes(4))
    );
    assert_eq!(bridge.write(generation, &[5]), Ok(Progress::Pending));
    let mut output = [0; 3];
    let ticket = bridge
        .network_send(generation, &mut output)
        .unwrap()
        .unwrap();
    assert_eq!(output, [0, 255, 128]);
    assert_eq!(ticket.len(), 3);
    assert!(!ticket.is_empty());
    assert_eq!(
        bridge.network_send(generation, &mut output),
        Err(Refusal::Busy)
    );
    // A provider cannot acknowledge bytes outside its exact copied view.
    assert_eq!(bridge.network_sent(ticket, 4), Err(Refusal::Extent));
    bridge.network_sent(ticket, 2).unwrap();
    assert_eq!(bridge.write(generation, &[4, 5]), Ok(Progress::Bytes(2)));
    assert_eq!(bridge.network_sent(ticket, 2), Err(Refusal::Stale));
    let mut rest = [0; 4];
    let next = bridge.network_send(generation, &mut rest).unwrap().unwrap();
    assert_eq!(rest, [128, 3, 4, 5]);
    bridge.cancel_send(next).unwrap();
    assert_eq!(bridge.network_sent(next, 1), Err(Refusal::Stale));
    let retry = bridge.network_send(generation, &mut rest).unwrap().unwrap();
    assert_eq!(rest, [128, 3, 4, 5]);
    bridge.network_sent(retry, 4).unwrap();
    assert_eq!(bridge.network_send(generation, &mut rest), Ok(None));
}
#[test]
fn receive_pressure_and_eof_preserve_accepted_bytes_before_terminal_read() {
    let mut bridge = SocketBridge::<4, 4>::new();
    let generation = open(&mut bridge);
    assert_eq!(bridge.readable(generation), Ok(false));
    assert_eq!(bridge.rx_room(generation), Ok(4));
    assert_eq!(
        bridge.network_receive(generation, &[0, 255, 128, 3, 4]),
        Ok(4)
    );
    assert_eq!(bridge.network_receive(generation, &[4]), Ok(0));
    assert_eq!(bridge.readable(generation), Ok(true));
    let mut first = [0; 2];
    assert_eq!(bridge.read(generation, &mut first), Ok(Progress::Bytes(2)));
    assert_eq!(first, [0, 255]);
    assert_eq!(bridge.network_receive(generation, &[4, 5]), Ok(2));
    bridge.peer_closed(generation).unwrap();
    let mut rest = [0; 8];
    assert_eq!(bridge.read(generation, &mut rest), Ok(Progress::Bytes(4)));
    assert_eq!(&rest[..4], &[128, 3, 4, 5]);
    assert_eq!(bridge.read(generation, &mut rest), Ok(Progress::Eof));
    assert_eq!(
        bridge.network_receive(generation, &[6]),
        Err(Refusal::State)
    );
}
#[test]
fn fault_does_not_silently_discard_the_already_accepted_receive_prefix() {
    let mut bridge = SocketBridge::<4, 4>::new();
    let generation = open(&mut bridge);
    bridge.network_receive(generation, &[9, 8]).unwrap();
    bridge.fail(generation, Fault::Network).unwrap();
    let mut bytes = [0; 4];
    assert_eq!(bridge.read(generation, &mut bytes), Ok(Progress::Bytes(2)));
    assert_eq!(&bytes[..2], &[9, 8]);
    assert_eq!(
        bridge.read(generation, &mut bytes),
        Err(Refusal::Fault(Fault::Network))
    );
    assert_eq!(
        bridge.write(generation, &[7]),
        Err(Refusal::Fault(Fault::Network))
    );
    close(&mut bridge, generation);
}
#[test]
fn close_requires_actual_provider_retirement_and_late_completions_cannot_resurrect() {
    let mut bridge = SocketBridge::<4, 4>::new();
    let old = bridge.request_connect(PEER, 100).unwrap();
    bridge.take_command(1).unwrap();
    bridge.request_close(old).unwrap();
    assert_eq!(bridge.complete_connect(old, 2, Ok(())), Err(Refusal::State));
    assert_eq!(bridge.request_connect(PEER, 100), Err(Refusal::Busy));
    bridge.take_command(3).unwrap();
    bridge.complete_close(old).unwrap();
    let current = open(&mut bridge);
    assert_ne!(current.get(), old.get());
    assert_eq!(bridge.generation(), Some(current));
    assert_eq!(bridge.complete_connect(old, 4, Ok(())), Err(Refusal::Stale));
    assert_eq!(bridge.network_receive(old, &[1]), Err(Refusal::Stale));
    assert_eq!(bridge.request_close(old), Err(Refusal::Stale));
    assert_eq!(bridge.phase(), Phase::Open);
}
#[test]
fn socket_reuse_drops_old_unsent_and_unread_bytes_and_old_send_completion() {
    let mut bridge = SocketBridge::<4, 4>::new();
    let old = open(&mut bridge);
    bridge.write(old, &[1, 2, 3]).unwrap();
    bridge.network_receive(old, &[9]).unwrap();
    let mut bytes = [0; 4];
    let ticket = bridge.network_send(old, &mut bytes).unwrap().unwrap();
    close(&mut bridge, old);
    let current = open(&mut bridge);
    bridge.write(current, &[4, 5]).unwrap();
    assert_eq!(bridge.network_sent(ticket, 2), Err(Refusal::Stale));
    let next = bridge.network_send(current, &mut bytes).unwrap().unwrap();
    assert_eq!(&bytes[..next.len()], &[4, 5]);
    assert_eq!(bridge.read(current, &mut bytes), Ok(Progress::Pending));
}
#[test]
fn preparation_deadlines_refuse_before_dispatch_and_after_a_late_success() {
    for taken in [false, true] {
        let mut bridge = SocketBridge::<4, 4>::new();
        let generation = bridge.request_connect(PEER, 10).unwrap();
        if taken {
            bridge.take_command(1).unwrap();
            assert_eq!(
                bridge.complete_connect(generation, 10, Ok(())),
                Ok(Phase::Failed(Fault::TimedOut))
            );
        } else {
            assert_eq!(bridge.take_command(10), None);
        }
        assert_eq!(bridge.phase(), Phase::Failed(Fault::TimedOut));
        assert_eq!(
            bridge.write(generation, &[1]),
            Err(Refusal::Fault(Fault::TimedOut))
        );
        close(&mut bridge, generation);
    }
}
#[test]
fn invalid_endpoint_and_repeated_commands_never_enlarge_or_replace_the_live_lease() {
    let mut bridge = SocketBridge::<4, 4>::default();
    assert_eq!(
        bridge.request_connect(Endpoint { port: 0, ..PEER }, 100),
        Err(Refusal::Endpoint)
    );
    assert_eq!(bridge.phase(), Phase::Idle);
    let generation = open(&mut bridge);
    assert_eq!(bridge.request_connect(PEER, 100), Err(Refusal::Busy));
    assert_eq!(
        bridge.complete_connect(generation, 3, Ok(())),
        Err(Refusal::State)
    );
    bridge.request_close(generation).unwrap();
    bridge.take_command(4).unwrap();
    bridge.request_close(generation).unwrap();
    assert_eq!(bridge.take_command(5), None);
    bridge.complete_close(generation).unwrap();
    assert_eq!(bridge.generation(), None);
}
