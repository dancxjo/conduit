use super::*;

fn storage() -> UsbDma {
    UsbDma {
        device_context: [0; 2048],
        input_context: [0; 2112],
        transfer_ring: [[0; 4]; TRANSFER_TRBS],
        descriptor: [0xcc; MAX_CONFIGURATION_BYTES],
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
