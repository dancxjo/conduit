use super::*;

fn storage() -> UsbDma {
    UsbDma {
        device_context: [0; 2048],
        input_context: [0; 2112],
        transfer_ring: [[0; 4]; TRANSFER_TRBS],
        descriptor: [0xcc; MAX_CONFIGURATION_BYTES],
        control_cursor: crate::usb_base::control_ring::ControlRingCursor::new(),
        owner_slot: 1,
        owner_root_port: 1,
        owner_epoch: 1,
    }
}

#[test]
fn output_data_is_bounded_and_preserves_neighboring_dma_storage() {
    let mut dma = storage();
    let request =
        ControlTransferRequest::new([0x22, 1, 0, 0, 0x81, 0, 3, 0], &[1, 2, 3], 256).unwrap();
    stage_output(&mut dma, &request).unwrap();
    assert_eq!(&dma.descriptor[..3], &[1, 2, 3]);
    assert!(dma.descriptor[3..].iter().all(|byte| *byte == 0xcc));
    assert_eq!(dma.transfer_ring, [[0; 4]; TRANSFER_TRBS]);
    assert_eq!(dma.device_context, [0; 2048]);
    assert_eq!(dma.input_context, [0; 2112]);
}

#[test]
fn broader_request_bounds_cannot_broaden_native_dma_geometry() {
    let mut dma = storage();
    let payload = [1; 257];
    let request =
        ControlTransferRequest::new([0, 0, 0, 0, 0, 0, 1, 1], &payload, u16::MAX).unwrap();
    assert_eq!(
        stage_output(&mut dma, &request),
        Err(UsbError::TransferEnvelope)
    );
    assert_eq!(dma.descriptor, [0xcc; MAX_CONFIGURATION_BYTES]);
    assert_eq!(dma.transfer_ring, [[0; 4]; TRANSFER_TRBS]);
}

#[test]
fn actual_dma_publication_reuses_a_fixed_ring_across_ten_thousand_transfers() {
    let mut dma = storage();
    let mut ring = ControlRing {
        physical: 0x1000,
        buffer_physical: 0x2000,
        root_port: 1,
        slot: 1,
        short_packets: 0,
        dma: &mut dma,
    };
    let request = ControlTransferRequest::new([0x80, 6, 0, 1, 0, 0, 8, 0], &[], 256).unwrap();
    let mut consumer = 0;
    let mut cycle = 1;
    let mut wraps = 0;
    for _ in 0..10_000 {
        let (reservation, mut completion) = prepare_transfer(&mut ring, &request).unwrap();
        if let Some((index, old_cycle)) = reservation.link {
            assert_eq!((consumer, cycle), (index, old_cycle));
            let link = dma.transfer_ring[consumer];
            assert_eq!(link, [0x1000, 0, 0, (6 << 10) | (1 << 1) | cycle]);
            consumer = 0;
            cycle ^= 1;
            wraps += 1;
        }
        assert_eq!(reservation.start, consumer);
        for (offset, kind) in [(0, 2), (1, 3), (2, 4)] {
            let trb = dma.transfer_ring[consumer + offset];
            assert_eq!((trb[3] >> 10) & 0x3f, kind);
            assert_eq!(trb[3] & 1, cycle);
        }
        // Model controller look-ahead past Status before the next doorbell.
        assert_ne!(
            dma.transfer_ring[consumer + reservation.count][3] & 1,
            cycle
        );
        let event = crate::arch::x86_64::xhci::Event {
            event_type: 32,
            completion_code: 1,
            slot: 1,
            endpoint: 1,
            residual: 0,
            pointer: 0x1000 + ((consumer + 2) * 16) as u64,
        };
        assert_eq!(completion.observe(event).unwrap().unwrap().bytes, 8);
        unsafe { dma.control_cursor.complete_quiesced(&reservation) }.unwrap();
        consumer += 3;
        assert_eq!(dma.control_cursor.position(), (consumer, cycle));
    }
    assert!(wraps > 900);
    assert_eq!(core::mem::size_of::<UsbDma>(), 8192);
}

#[test]
fn pending_or_uncertain_td_refuses_before_mutating_dma_payload_or_ring() {
    let mut dma = storage();
    let mut ring = ControlRing {
        physical: 0x1000,
        buffer_physical: 0x2000,
        root_port: 1,
        slot: 1,
        short_packets: 0,
        dma: &mut dma,
    };
    let first = ControlTransferRequest::new([0, 1, 0, 0, 0, 0, 3, 0], &[1, 2, 3], 256).unwrap();
    let (_pending, _completion) = prepare_transfer(&mut ring, &first).unwrap();
    let trbs = dma.transfer_ring;
    let payload = dma.descriptor;
    let next = ControlTransferRequest::new([0, 1, 0, 0, 0, 0, 3, 0], &[4, 5, 6], 256).unwrap();
    assert!(matches!(
        prepare_transfer(&mut ring, &next),
        Err(UsbError::TransferRingFull)
    ));
    assert_eq!(dma.transfer_ring, trbs);
    assert_eq!(dma.descriptor, payload);
    dma.control_cursor.retain_uncertain();
    assert!(matches!(
        prepare_transfer(&mut ring, &next),
        Err(UsbError::TransferRingUncertain)
    ));
    assert_eq!(dma.transfer_ring, trbs);
    assert_eq!(dma.descriptor, payload);
}
