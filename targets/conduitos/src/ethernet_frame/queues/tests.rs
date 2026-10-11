use super::*;
fn frame(length: usize) -> alloc::vec::Vec<u8> {
    let mut frame = alloc::vec![0;length];
    if length >= 14 {
        frame[..6].fill(0xff);
        frame[6..12].copy_from_slice(&[2, 1, 2, 3, 4, 5]);
        frame[12..14].copy_from_slice(&[8, 0]);
    }
    frame
}
#[test]
fn exact_ethernet_ii_bounds_and_header_are_required() {
    for length in [0, 1, 13, 1515] {
        assert_eq!(validate_frame(&frame(length)), Err(FrameRefusal::Length));
    }
    for length in [14, 64, 1514] {
        validate_frame(&frame(length)).unwrap();
    }
    let mut wire = frame(64);
    wire[6..12].fill(0);
    assert_eq!(validate_frame(&wire), Err(FrameRefusal::SourceMac));
    wire[6] = 3;
    assert_eq!(validate_frame(&wire), Err(FrameRefusal::SourceMac));
    wire[6] = 2;
    wire[12..14].copy_from_slice(&1500u16.to_be_bytes());
    assert_eq!(validate_frame(&wire), Err(FrameRefusal::EtherType));
    wire[12..14].copy_from_slice(&0x8100u16.to_be_bytes());
    assert_eq!(validate_frame(&wire), Err(FrameRefusal::EtherType));
}
#[test]
fn pending_transmit_is_not_replayed_or_freed_by_short_or_foreign_ack() {
    let queues = EthernetFrameQueues::prepare(7, [2, 1, 2, 3, 4, 5]).unwrap();
    let wire = frame(64);
    let mut output = [0; FRAME_BYTES];
    queues.transmit_ready(7).unwrap();
    queues.enqueue_transmit(7, &wire).unwrap();
    queues.enqueue_transmit(7, &wire).unwrap();
    assert_eq!(queues.transmit_ready(7), Err(FrameRefusal::Pressure));
    assert_eq!(
        queues.enqueue_transmit(7, &wire),
        Err(FrameRefusal::Pressure)
    );
    assert_eq!(queues.take_transmit(7, &mut output).unwrap(), (0, 64));
    assert_eq!(&output[..64], wire);
    assert_eq!(
        queues.take_transmit(7, &mut output),
        Err(FrameRefusal::Pressure)
    );
    assert_eq!(
        queues.acknowledge_transmit(7, 0, 63),
        Err(FrameRefusal::Length)
    );
    assert_eq!(
        queues.acknowledge_transmit(7, 1, 64),
        Err(FrameRefusal::Stale)
    );
    assert_eq!(
        queues.acknowledge_transmit(8, 0, 64),
        Err(FrameRefusal::Stale)
    );
    assert_eq!(
        queues.enqueue_transmit(7, &wire),
        Err(FrameRefusal::Pressure)
    );
    queues.acknowledge_transmit(7, 0, 64).unwrap();
    assert_eq!(
        queues.acknowledge_transmit(7, 0, 64),
        Err(FrameRefusal::Stale)
    );
    assert_eq!(queues.take_transmit(7, &mut output).unwrap(), (1, 64));
}
#[test]
fn sustained_bidirectional_reuse_and_cancel_have_no_play_allocations() {
    let queues = EthernetFrameQueues::prepare(7, [2, 1, 2, 3, 4, 5]).unwrap();
    let wire = frame(1514);
    let mut tx = [0; FRAME_BYTES];
    let mut rx = [0; FRAME_BYTES];
    let allocations = crate::test_allocations::allocations(|| {
        for ordinal in 0..1024 {
            queues.enqueue_transmit(7, &wire).unwrap();
            let (actual, length) = queues.take_transmit(7, &mut tx).unwrap();
            assert_eq!(actual, ordinal);
            assert_eq!(length, 1514);
            queues.acknowledge_transmit(7, ordinal, 1514).unwrap();
            queues.deliver_receive(7, &wire).unwrap();
            assert_eq!(queues.receive(7, &mut rx).unwrap(), 1514);
            assert_eq!(tx, rx);
        }
        queues.cancel();
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        queues.enqueue_transmit(7, &wire),
        Err(FrameRefusal::Cancelled)
    );
    assert_eq!(
        queues.acknowledge_transmit(7, 1023, 1514),
        Err(FrameRefusal::Cancelled)
    );
}
#[test]
fn receive_pressure_and_provider_loss_are_distinct_from_generation_and_cancel() {
    let queues = EthernetFrameQueues::prepare(7, [2, 1, 2, 3, 4, 5]).unwrap();
    let wire = frame(64);
    let mut out = [0; FRAME_BYTES];
    assert_eq!(queues.receive(7, &mut out), Err(FrameRefusal::Pressure));
    queues.deliver_receive(7, &wire).unwrap();
    queues.deliver_receive(7, &wire).unwrap();
    assert_eq!(
        queues.deliver_receive(7, &wire),
        Err(FrameRefusal::Pressure)
    );
    assert_eq!(queues.deliver_receive(8, &wire), Err(FrameRefusal::Stale));
    queues.provider_lost();
    assert_eq!(queues.receive(7, &mut out), Err(FrameRefusal::ProviderLost));
    queues.cancel();
    assert_eq!(queues.receive(7, &mut out), Err(FrameRefusal::Cancelled));
}

#[test]
fn invalid_generation_and_sequence_exhaustion_cannot_release_or_publish_a_frame() {
    assert!(matches!(
        EthernetFrameQueues::prepare(0, [2, 1, 2, 3, 4, 5]),
        Err(FrameRefusal::Stale)
    ));
    let queues = EthernetFrameQueues::prepare(7, [2, 1, 2, 3, 4, 5]).unwrap();
    let wire = frame(64);
    queues.enqueue_transmit(7, &wire).unwrap();
    queues.state.borrow_mut().next = u64::MAX;
    let mut output = [0; FRAME_BYTES];
    assert_eq!(
        queues.take_transmit(7, &mut output),
        Err(FrameRefusal::Sequence)
    );
    let state = queues.state.borrow();
    assert_eq!(state.tx.count, 1);
    assert!(state.pending.is_none());
    assert_eq!(output, [0; FRAME_BYTES]);
    drop(state);
    queues.cancel();
    queues.provider_lost();
    assert_eq!(queues.receive(7, &mut output), Err(FrameRefusal::Cancelled));
}
