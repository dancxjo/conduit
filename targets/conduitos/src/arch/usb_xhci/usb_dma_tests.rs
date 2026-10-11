use super::*;

#[test]
fn dma_storage_rejects_stale_or_unowned_device_attachments() {
    let bytes = [18, 1, 0, 2, 0, 0, 0, 8, 1, 0, 2, 0, 0, 1, 0, 0, 0, 1];
    let mut device = super::super::descriptor::device_from_descriptor(1, 1, 1, 8, &bytes).unwrap();
    device.attachment_epoch = 3;
    let mut dma = EMPTY_DMA;
    assert!(!dma.matches_device(&device));
    dma.owner_slot = 1;
    dma.owner_root_port = 1;
    dma.owner_epoch = 3;
    assert!(dma.matches_device(&device));
    for change in [
        |d: &mut UsbDevice| d.slot = 2,
        |d: &mut UsbDevice| d.root_port = 2,
        |d: &mut UsbDevice| d.attachment_epoch = 2,
        |d: &mut UsbDevice| d.attachment_epoch = 0,
    ] {
        let mut stale =
            super::super::descriptor::device_from_descriptor(1, 1, 1, 8, &bytes).unwrap();
        stale.attachment_epoch = 3;
        change(&mut stale);
        assert!(!dma.matches_device(&stale));
    }
    dma.owner_slot = 0;
    assert!(!dma.matches_device(&device));
    device.slot = 0;
    assert!(!dma.matches_device(&device));
}
