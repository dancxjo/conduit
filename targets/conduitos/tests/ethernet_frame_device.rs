//! Same smoltcp Device API; fixed queue pressure, no service or USB proof.
#![cfg(feature = "native-owner-network")]
use conduitos::ethernet_frame::{
    device::EthernetFrameDevice,
    queues::{EthernetFrameQueues, FRAME_BYTES, FrameRefusal},
};
use smoltcp::{
    phy::{Device, Medium, TxToken},
    time::Instant,
};

#[test]
fn malformed_stack_frames_do_not_consume_queue_space_and_full_tx_stops_tokens() {
    let queues = EthernetFrameQueues::prepare(7, [2, 1, 2, 3, 4, 5]).unwrap();
    let mut device = EthernetFrameDevice::prepare(&queues);
    assert_eq!(device.capabilities().medium, Medium::Ethernet);
    assert_eq!(device.capabilities().max_transmission_unit, FRAME_BYTES);
    for length in [0, 13, FRAME_BYTES + 1] {
        device
            .transmit(Instant::ZERO)
            .unwrap()
            .consume(length, |_| ());
        assert_eq!(device.take_error(), Some(FrameRefusal::Length));
    }
    device.transmit(Instant::ZERO).unwrap().consume(14, |_| ());
    assert_eq!(device.take_error(), Some(FrameRefusal::SourceMac));
    for length in [14, FRAME_BYTES] {
        device
            .transmit(Instant::ZERO)
            .unwrap()
            .consume(length, |wire| {
                wire[..6].fill(0xff);
                wire[6..12].copy_from_slice(&queues.mac());
                wire[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
            });
        assert_eq!(device.take_error(), None);
    }
    assert!(device.transmit(Instant::ZERO).is_none());
    assert!(device.receive(Instant::ZERO).is_none());
    assert_eq!(device.take_error(), None);
}
