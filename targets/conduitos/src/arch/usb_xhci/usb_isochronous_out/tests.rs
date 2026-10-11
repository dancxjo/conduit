use super::*;
use crate::usb_base::endpoint_ring::EndpointRingCursor;
fn transfer<'a>(
    ring: &'a mut [[u32; 4]; 64],
    buffer: &'a mut [u8; 2048],
    cursor: &'a mut EndpointRingCursor,
) -> IsochronousOutTransfer<'a> {
    IsochronousOutTransfer {
        dma: EndpointTransmitDma {
            ring,
            buffer,
            cursor,
            ring_physical: 0x10000,
            buffer_physical: 0x20000,
        },
        geometry: IsochronousOutGeometry::usb2(1, 256, 1).unwrap(),
        slot: 1,
        attachment_epoch: 1,
        endpoint_epoch: 1,
        dci: 2,
        pending: core::array::from_fn(|_| None),
        head: 0,
        count: 0,
        next_frame: None,
        faulted: false,
    }
}
fn completion(transfer: &IsochronousOutTransfer<'_>, code: u8) -> Event {
    let pending = transfer.pending[transfer.head].as_ref().unwrap();
    Event {
        event_type: 32,
        completion_code: code,
        slot: 1,
        endpoint: 2,
        residual: 0,
        pointer: transfer.dma.ring_physical + (pending.reservation.slot * 16) as u64,
    }
}
#[test]
fn eight_reserved_packets_preserve_payloads_and_pressure_without_mutation() {
    let mut ring = [[0; 4]; 64];
    let mut buffer = [0; 2048];
    let mut cursor = EndpointRingCursor::with_maximum_pending(64, 8).unwrap();
    let mut transfer = transfer(&mut ring, &mut buffer, &mut cursor);
    for index in 0..8 {
        transfer
            .begin(
                IsochronousFrame::new(index + 8).unwrap(),
                &[index as u8; 192],
                0,
                0,
            )
            .unwrap();
    }
    assert_eq!(transfer.pending_count(), 8);
    for index in 0..8 {
        assert_eq!(
            &transfer.dma.buffer[index * 256..index * 256 + 192],
            &[index as u8; 192]
        );
    }
    let before = *transfer.dma.ring;
    assert!(matches!(
        transfer.begin(IsochronousFrame::new(16).unwrap(), &[99; 192], 0, 0),
        Err(IsochronousOutRefusal::Pending)
    ));
    assert_eq!(*transfer.dma.ring, before);
    for _ in 0..8 {
        let event = completion(&transfer, 1);
        assert!(matches!(
            transfer.event(event),
            Ok(IsochronousOutObservation::Completed { .. })
        ));
    }
    assert_eq!(transfer.dma.cursor.pending_count(), 0);
}
#[test]
fn ten_thousand_exact_iso_completions_reuse_ring_cycles_and_fixed_storage() {
    let mut ring = [[0; 4]; 64];
    let mut buffer = [0; 2048];
    let mut cursor = EndpointRingCursor::with_maximum_pending(64, 8).unwrap();
    let mut transfer = transfer(&mut ring, &mut buffer, &mut cursor);
    for index in 0_u16..10000 {
        let frame = (index + 8) & 2047;
        let now = (index & 2047) << 3;
        transfer
            .begin(
                IsochronousFrame::new(frame).unwrap(),
                &[index as u8; 192],
                now,
                0,
            )
            .unwrap();
        let pending = transfer.pending[transfer.head].as_ref().unwrap();
        let trb = transfer.dma.ring[pending.reservation.slot];
        assert_eq!((trb[3] >> 10) & 63, 5);
        assert_eq!((trb[3] >> 20) & 2047, u32::from(frame));
        assert_eq!((trb[3] & 1), pending.reservation.cycle);
        let event = completion(&transfer, 1);
        transfer.event(event).unwrap();
    }
    assert_eq!(transfer.dma.cursor.pending_count(), 0);
    assert_eq!(
        transfer.dma.cursor.position(),
        (10000 % 63, 1 ^ ((10000 / 63) % 2) as u32)
    );
}
#[test]
fn foreign_events_and_underrun_do_not_release_pending_buffers() {
    let mut ring = [[0; 4]; 64];
    let mut buffer = [0; 2048];
    let mut cursor = EndpointRingCursor::with_maximum_pending(64, 8).unwrap();
    let mut transfer = transfer(&mut ring, &mut buffer, &mut cursor);
    transfer
        .begin(IsochronousFrame::new(8).unwrap(), &[3; 192], 0, 0)
        .unwrap();
    let mut event = completion(&transfer, 1);
    event.pointer += 16;
    assert!(matches!(
        transfer.event(event),
        Err(IsochronousOutRefusal::ForeignCompletion)
    ));
    event.completion_code = 14;
    assert_eq!(
        transfer.event(event).unwrap(),
        IsochronousOutObservation::Underrun
    );
    assert_eq!(transfer.dma.cursor.pending_count(), 1);
    assert!(matches!(
        transfer.begin(IsochronousFrame::new(9).unwrap(), &[4; 192], 0, 0),
        Err(IsochronousOutRefusal::Faulted)
    ));
    // Fixture acknowledgement: event alone did not reclaim any lease.
    unsafe { transfer.release_stopped() }.unwrap();
    assert_eq!(transfer.dma.cursor.pending_count(), 0);
}
#[test]
fn missed_service_retires_only_its_exact_td_and_quarantines_remaining_dma() {
    let mut ring = [[0; 4]; 64];
    let mut buffer = [0; 2048];
    let mut cursor = EndpointRingCursor::with_maximum_pending(64, 8).unwrap();
    let mut transfer = transfer(&mut ring, &mut buffer, &mut cursor);
    transfer
        .begin(IsochronousFrame::new(8).unwrap(), &[3; 192], 0, 0)
        .unwrap();
    transfer
        .begin(IsochronousFrame::new(9).unwrap(), &[4; 192], 0, 0)
        .unwrap();
    let event = completion(&transfer, 23);
    assert_eq!(
        transfer.event(event).unwrap(),
        IsochronousOutObservation::MissedService { frame: 8 }
    );
    assert_eq!(transfer.dma.cursor.pending_count(), 1);
    assert_eq!(&transfer.dma.buffer[256..448], &[4; 192]);
}
#[test]
fn late_or_noncontiguous_deadlines_refuse_before_dma_or_ring_mutation() {
    let mut ring = [[0; 4]; 64];
    let mut buffer = [0; 2048];
    let mut cursor = EndpointRingCursor::with_maximum_pending(64, 8).unwrap();
    let mut transfer = transfer(&mut ring, &mut buffer, &mut cursor);
    assert!(
        transfer
            .begin(IsochronousFrame::new(0).unwrap(), &[3; 192], 0, 0)
            .is_err()
    );
    assert_eq!(transfer.dma.cursor.position(), (0, 1));
    assert_eq!(*transfer.dma.buffer, [0; 2048]);
    transfer
        .begin(IsochronousFrame::new(8).unwrap(), &[3; 192], 0, 0)
        .unwrap();
    assert!(
        transfer
            .begin(IsochronousFrame::new(10).unwrap(), &[4; 192], 0, 0)
            .is_err()
    );
    assert_eq!(transfer.dma.cursor.pending_count(), 1);
}
