//! Three fixed, independently addressed USB enumeration DMA slots.

use super::{MAX_CONFIGURATION_BYTES, TRANSFER_TRBS, UsbDevice, UsbError};

pub const USB_DEVICE_DMA_SLOTS: u8 = 3;

#[repr(C, align(4096))]
pub(in crate::arch::x86_64) struct UsbDma {
    pub(in crate::arch::x86_64) device_context: [u8; 2048],
    pub(super) input_context: [u8; 2112],
    pub(super) transfer_ring: [[u32; 4]; TRANSFER_TRBS],
    pub(super) descriptor: [u8; MAX_CONFIGURATION_BYTES],
}

const EMPTY_DMA: UsbDma = UsbDma {
    device_context: [0; 2048],
    input_context: [0; 2112],
    transfer_ring: [[0; 4]; TRANSFER_TRBS],
    descriptor: [0; MAX_CONFIGURATION_BYTES],
};

static mut PRIMARY_DMA: UsbDma = EMPTY_DMA;
static mut SECONDARY_DMA: UsbDma = EMPTY_DMA;
static mut TERTIARY_DMA: UsbDma = EMPTY_DMA;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UsbDmaSlot(u8);

impl UsbDmaSlot {
    pub(super) const PRIMARY: Self = Self(0);
    pub(super) const SECONDARY: Self = Self(1);
    pub(super) const TERTIARY: Self = Self(2);

    pub(super) const fn index(self) -> u8 {
        self.0
    }

    pub(super) const fn from_index(index: u8) -> Result<Self, UsbError> {
        if index < USB_DEVICE_DMA_SLOTS {
            Ok(Self(index))
        } else {
            Err(UsbError::DmaSlotInvalid)
        }
    }
}

pub(super) fn dma_pointer(slot: UsbDmaSlot) -> *mut UsbDma {
    match slot.0 {
        0 => core::ptr::addr_of_mut!(PRIMARY_DMA),
        1 => core::ptr::addr_of_mut!(SECONDARY_DMA),
        2 => core::ptr::addr_of_mut!(TERTIARY_DMA),
        _ => unreachable!(),
    }
}

pub(in crate::arch::x86_64) fn device_dma_pointer(
    device: &UsbDevice,
) -> Result<*mut UsbDma, UsbError> {
    Ok(dma_pointer(UsbDmaSlot::from_index(device.dma_slot)?))
}
