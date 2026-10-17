use super::*;

#[test]
fn selected_dma_requires_every_fixed_page_and_the_last_byte_to_map_contiguously() {
    assert_eq!(
        mapped_dma_physical(0x1000, |address| address.checked_add(0x100000)),
        Ok(0x101000)
    );
    for mapping in [
        (|_| None) as fn(u64) -> Option<u64>,
        |address| address.checked_add(1),
        |address| (address != 0x2000).then_some(address + 0x100000),
        |address| {
            Some(
                address
                    + if address == 0x2000 {
                        0x200000
                    } else {
                        0x100000
                    },
            )
        },
        |address| (address != 0x2fff).then_some(address + 0x100000),
    ] {
        assert_eq!(
            mapped_dma_physical(0x1000, mapping),
            Err(UsbError::DmaAddressInvalid)
        );
    }
}

#[test]
fn virtual_physical_overflow_and_misalignment_refuse_without_dereferencing_memory() {
    assert_eq!(
        mapped_dma_physical(0x1001, Some),
        Err(UsbError::DmaAddressInvalid)
    );
    assert_eq!(
        mapped_dma_physical(u64::MAX - 4095, Some),
        Err(UsbError::DmaAddressInvalid)
    );
    assert_eq!(
        mapped_dma_physical(0x1000, |_| Some(u64::MAX - 4095)),
        Err(UsbError::DmaAddressInvalid)
    );
}
