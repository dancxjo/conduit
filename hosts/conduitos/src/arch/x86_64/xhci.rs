//! One fixed, boot-local xHCI controller Base for the pinned Q35 proof.
//!
//! This module owns PCI discovery and xHCI mechanism only. It does not infer a
//! USB device, HID interface, or semantic input capability.

use core::{
    arch::asm,
    hint::spin_loop,
    ptr::{read_volatile, write_volatile},
};

use super::io::{inl, outl};

const PCI_CONFIG_ADDRESS: u16 = 0x0cf8;
const PCI_CONFIG_DATA: u16 = 0x0cfc;
const XHCI_CLASS: u32 = 0x0c03_3000;
const COMMAND_TRBS: usize = 16;
const EVENT_TRBS: usize = 16;
const ADMITTED_DEVICE_SLOTS: u8 = 1;
const MAX_PENDING_COMMANDS: u8 = 1;
const POLL_STEPS: u32 = 2_000_000;
const SIGN_SLOTS: u8 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XhciError {
    Absent,
    WrongClass,
    InvalidBar,
    InvalidLayout,
    UnsupportedPageSize,
    ScratchpadsUnsupported,
    ResetTimeout,
    StartTimeout,
    CommandRingFull,
    UnexpectedCompletion,
    CommandTimeout,
    DmaAddressInvalid,
}

impl XhciError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "xhci-controller-absent",
            Self::WrongClass => "xhci-wrong-pci-class",
            Self::InvalidBar => "xhci-invalid-bar",
            Self::InvalidLayout => "xhci-invalid-register-layout",
            Self::UnsupportedPageSize => "xhci-unsupported-page-size",
            Self::ScratchpadsUnsupported => "xhci-scratchpads-unsupported",
            Self::ResetTimeout => "xhci-reset-timeout",
            Self::StartTimeout => "xhci-start-timeout",
            Self::CommandRingFull => "xhci-command-ring-full",
            Self::UnexpectedCompletion => "xhci-unexpected-completion",
            Self::CommandTimeout => "xhci-command-timeout",
            Self::DmaAddressInvalid => "xhci-dma-address-invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XhciReady {
    pub segment: u8,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor: u16,
    pub device_id: u16,
    pub bar_physical: u64,
    pub hardware_slots: u8,
    pub admitted_slots: u8,
    pub command_trbs: u8,
    pub event_trbs: u8,
    pub dma_bytes: u16,
    pub dma_alignment: u16,
    pub maximum_pending_commands: u8,
    pub poll_steps: u32,
    pub sign_slots: u8,
}

#[repr(C, align(64))]
struct DmaStorage {
    dcbaa: [u64; 2],
    _dcbaa_padding: [u8; 48],
    command_ring: [[u32; 4]; COMMAND_TRBS],
    event_ring: [[u32; 4]; EVENT_TRBS],
    erst: [u64; 2],
    _erst_padding: [u8; 48],
}

static mut DMA: DmaStorage = DmaStorage {
    dcbaa: [0; 2],
    _dcbaa_padding: [0; 48],
    command_ring: [[0; 4]; COMMAND_TRBS],
    event_ring: [[0; 4]; EVENT_TRBS],
    erst: [0; 2],
    _erst_padding: [0; 48],
};

#[repr(C, align(4096))]
struct PageTable([u64; 512]);

static mut MMIO_PDPT: PageTable = PageTable([0; 512]);
static mut MMIO_PD: PageTable = PageTable([0; 512]);
const MMIO_PML4_INDEX: usize = 509;
const MMIO_VIRTUAL_BASE: u64 = 0xffff_fe80_0000_0000;

pub fn initialize_xhci(
    hhdm: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<XhciReady, XhciError> {
    let pci = discover()?;
    let mmio = map_mmio(pci.bar, hhdm, image_virtual_to_physical)?;
    let dma_virtual = core::ptr::addr_of_mut!(DMA) as u64;
    let dma_physical =
        image_virtual_to_physical(dma_virtual).ok_or(XhciError::DmaAddressInvalid)?;
    if dma_physical & 63 != 0 {
        return Err(XhciError::DmaAddressInvalid);
    }
    let hardware_slots = unsafe { initialize_registers(mmio, dma_physical)? };
    Ok(XhciReady {
        segment: 0,
        bus: pci.bus,
        device: pci.device,
        function: pci.function,
        vendor: pci.vendor,
        device_id: pci.device_id,
        bar_physical: pci.bar,
        hardware_slots,
        admitted_slots: ADMITTED_DEVICE_SLOTS,
        command_trbs: COMMAND_TRBS as u8,
        event_trbs: EVENT_TRBS as u8,
        dma_bytes: core::mem::size_of::<DmaStorage>() as u16,
        dma_alignment: core::mem::align_of::<DmaStorage>() as u16,
        maximum_pending_commands: MAX_PENDING_COMMANDS,
        poll_steps: POLL_STEPS,
        sign_slots: SIGN_SLOTS,
    })
}

fn map_mmio(
    physical: u64,
    hhdm: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<usize, XhciError> {
    let page_offset = physical & 0x1f_ffff;
    let physical_page = physical & !0x1f_ffff;
    let pdpt_physical = image_virtual_to_physical(core::ptr::addr_of!(MMIO_PDPT) as u64)
        .ok_or(XhciError::DmaAddressInvalid)?;
    let pd_physical = image_virtual_to_physical(core::ptr::addr_of!(MMIO_PD) as u64)
        .ok_or(XhciError::DmaAddressInvalid)?;
    if pdpt_physical & 0xfff != 0 || pd_physical & 0xfff != 0 {
        return Err(XhciError::DmaAddressInvalid);
    }
    let cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3, options(nostack, nomem, preserves_flags));
    }
    let pml4_virtual = hhdm
        .checked_add(cr3 & !0xfff)
        .ok_or(XhciError::InvalidLayout)?;
    let pml4 = usize::try_from(pml4_virtual).map_err(|_| XhciError::InvalidLayout)? as *mut u64;
    unsafe {
        MMIO_PDPT.0 = [0; 512];
        MMIO_PD.0 = [0; 512];
        MMIO_PDPT.0[0] = pd_physical | 0x3;
        MMIO_PD.0[0] = physical_page | 0x9b;
        write_volatile(pml4.add(MMIO_PML4_INDEX), pdpt_physical | 0x3);
        asm!("mov cr3, {}", in(reg) cr3, options(nostack, preserves_flags));
    }
    usize::try_from(MMIO_VIRTUAL_BASE + page_offset).map_err(|_| XhciError::InvalidLayout)
}

struct PciController {
    bus: u8,
    device: u8,
    function: u8,
    vendor: u16,
    device_id: u16,
    bar: u64,
}

fn discover() -> Result<PciController, XhciError> {
    let mut saw_usb = false;
    for bus in 0..=u8::MAX {
        for device in 0..32 {
            for function in 0..8 {
                let id = pci_read(bus, device, function, 0);
                if id & 0xffff == 0xffff {
                    continue;
                }
                let class = pci_read(bus, device, function, 8) & 0xffff_ff00;
                saw_usb |= class >> 16 == 0x0c03;
                if class != XHCI_CLASS {
                    continue;
                }
                let command = pci_read(bus, device, function, 4);
                pci_write(bus, device, function, 4, command | 0x6);
                let low = pci_read(bus, device, function, 0x10);
                if low & 1 != 0 || low & 0xffff_fff0 == 0 {
                    return Err(XhciError::InvalidBar);
                }
                let bar = if low & 0x6 == 0x4 {
                    u64::from(low & 0xffff_fff0)
                        | (u64::from(pci_read(bus, device, function, 0x14)) << 32)
                } else {
                    u64::from(low & 0xffff_fff0)
                };
                return Ok(PciController {
                    bus,
                    device,
                    function,
                    vendor: id as u16,
                    device_id: (id >> 16) as u16,
                    bar,
                });
            }
        }
    }
    Err(if saw_usb {
        XhciError::WrongClass
    } else {
        XhciError::Absent
    })
}

unsafe fn initialize_registers(mmio: usize, dma_physical: u64) -> Result<u8, XhciError> {
    let cap_length = unsafe { read8(mmio) } as usize;
    let hcs1 = unsafe { read32(mmio + 4) };
    let hcs2 = unsafe { read32(mmio + 8) };
    let hardware_slots = (hcs1 & 0xff) as u8;
    let scratchpads = (((hcs2 >> 27) & 0x1f) << 5) | ((hcs2 >> 21) & 0x1f);
    let doorbells = (unsafe { read32(mmio + 0x14) } & !3) as usize;
    let runtime = (unsafe { read32(mmio + 0x18) } & !0x1f) as usize;
    if !(0x20..=0x80).contains(&cap_length)
        || hardware_slots == 0
        || doorbells < 0x100
        || runtime < 0x100
    {
        return Err(XhciError::InvalidLayout);
    }
    if scratchpads != 0 {
        return Err(XhciError::ScratchpadsUnsupported);
    }
    let operational = mmio
        .checked_add(cap_length)
        .ok_or(XhciError::InvalidLayout)?;
    let runtime = mmio.checked_add(runtime).ok_or(XhciError::InvalidLayout)?;
    let doorbell = mmio
        .checked_add(doorbells)
        .ok_or(XhciError::InvalidLayout)?;
    unsafe {
        write32(operational, read32(operational) & !1);
    }
    unsafe {
        poll32(operational + 4, 1, 1, XhciError::ResetTimeout)?;
    }
    unsafe {
        write32(operational, read32(operational) | 2);
    }
    unsafe {
        poll32(operational, 2, 0, XhciError::ResetTimeout)?;
    }
    unsafe {
        poll32(operational + 4, 1 << 11, 0, XhciError::ResetTimeout)?;
    }
    if unsafe { read32(operational + 8) } & 1 == 0 {
        return Err(XhciError::UnsupportedPageSize);
    }

    let dma = core::ptr::addr_of_mut!(DMA);
    unsafe {
        (*dma).command_ring = [[0; 4]; COMMAND_TRBS];
        (*dma).event_ring = [[0; 4]; EVENT_TRBS];
    }
    let command_phys = dma_physical + core::mem::offset_of!(DmaStorage, command_ring) as u64;
    let event_phys = dma_physical + core::mem::offset_of!(DmaStorage, event_ring) as u64;
    let erst_phys = dma_physical + core::mem::offset_of!(DmaStorage, erst) as u64;
    let dcbaa_phys = dma_physical + core::mem::offset_of!(DmaStorage, dcbaa) as u64;
    unsafe {
        (*dma).command_ring[COMMAND_TRBS - 1] = [
            command_phys as u32,
            (command_phys >> 32) as u32,
            0,
            (6 << 10) | 3,
        ];
    }
    unsafe {
        (*dma).erst = [event_phys, EVENT_TRBS as u64];
        (*dma).dcbaa = [0; 2];
    }
    unsafe {
        write64(operational + 0x18, command_phys | 1);
        write64(operational + 0x30, dcbaa_phys);
        write32(operational + 0x38, 1);
    }
    let interrupter = runtime + 0x20;
    unsafe {
        write32(interrupter + 8, 1);
        write64(interrupter + 0x10, erst_phys);
        write64(interrupter + 0x18, event_phys);
    }
    unsafe {
        write32(operational, read32(operational) | 1);
        poll32(operational + 4, 1, 0, XhciError::StartTimeout)?;
    }

    unsafe {
        (*dma).command_ring[0] = [0, 0, 0, (23 << 10) | 1];
        write32(doorbell, 0);
    }
    for _ in 0..POLL_STEPS {
        let control = unsafe { read_volatile(core::ptr::addr_of!((*dma).event_ring[0][3])) };
        if control & 1 != 0 {
            let event_type = (control >> 10) & 0x3f;
            let completion =
                unsafe { read_volatile(core::ptr::addr_of!((*dma).event_ring[0][2])) } >> 24;
            let pointer =
                u64::from(unsafe { read_volatile(core::ptr::addr_of!((*dma).event_ring[0][0])) })
                    | (u64::from(unsafe {
                        read_volatile(core::ptr::addr_of!((*dma).event_ring[0][1]))
                    }) << 32);
            if event_type != 33 || completion != 1 || pointer != command_phys {
                return Err(XhciError::UnexpectedCompletion);
            }
            unsafe {
                write64(interrupter + 0x18, (event_phys + 16) | 8);
            }
            return Ok(hardware_slots);
        }
        spin_loop();
    }
    Err(XhciError::CommandTimeout)
}

unsafe fn poll32(
    address: usize,
    mask: u32,
    expected: u32,
    error: XhciError,
) -> Result<(), XhciError> {
    for _ in 0..POLL_STEPS {
        if unsafe { read32(address) } & mask == expected {
            return Ok(());
        }
        spin_loop();
    }
    Err(error)
}

fn pci_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xfc)
}
fn pci_read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    unsafe {
        outl(
            PCI_CONFIG_ADDRESS,
            pci_address(bus, device, function, offset),
        );
        inl(PCI_CONFIG_DATA)
    }
}
fn pci_write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    unsafe {
        outl(
            PCI_CONFIG_ADDRESS,
            pci_address(bus, device, function, offset),
        );
        outl(PCI_CONFIG_DATA, value);
    }
}
unsafe fn read8(address: usize) -> u8 {
    unsafe { read_volatile(address as *const u8) }
}
unsafe fn read32(address: usize) -> u32 {
    unsafe { read_volatile(address as *const u32) }
}
unsafe fn write32(address: usize, value: u32) {
    unsafe { write_volatile(address as *mut u32, value) }
}
unsafe fn write64(address: usize, value: u64) {
    unsafe { write_volatile(address as *mut u64, value) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pci_coordinates_are_exact() {
        assert_eq!(pci_address(2, 3, 1, 0x14), 0x8002_1914);
    }
    #[test]
    fn admitted_limits_do_not_follow_hardware_maxima() {
        assert_eq!(ADMITTED_DEVICE_SLOTS, 1);
        assert_eq!(MAX_PENDING_COMMANDS, 1);
    }
    #[test]
    fn dma_shape_is_fixed_and_aligned() {
        assert_eq!(core::mem::align_of::<DmaStorage>(), 64);
        assert_eq!(core::mem::size_of::<DmaStorage>(), 640);
        assert_eq!(core::mem::offset_of!(DmaStorage, dcbaa) % 64, 0);
        assert_eq!(core::mem::offset_of!(DmaStorage, command_ring) % 64, 0);
        assert_eq!(core::mem::offset_of!(DmaStorage, event_ring) % 64, 0);
        assert_eq!(core::mem::offset_of!(DmaStorage, erst) % 64, 0);
    }

    #[test]
    fn pci_and_bar_failures_are_distinct() {
        assert_ne!(XhciError::Absent, XhciError::WrongClass);
        assert_ne!(XhciError::WrongClass, XhciError::InvalidBar);
        assert_ne!(XhciError::InvalidBar, XhciError::InvalidLayout);
    }

    #[test]
    fn bounded_progress_failures_are_distinct() {
        assert_ne!(XhciError::ResetTimeout, XhciError::StartTimeout);
        assert_ne!(XhciError::CommandRingFull, XhciError::CommandTimeout);
        assert_ne!(XhciError::CommandTimeout, XhciError::UnexpectedCompletion);
    }

    #[test]
    fn unsupported_storage_and_page_shapes_fail_separately() {
        assert_ne!(
            XhciError::UnsupportedPageSize,
            XhciError::ScratchpadsUnsupported
        );
        assert_ne!(
            XhciError::ScratchpadsUnsupported,
            XhciError::DmaAddressInvalid
        );
    }

    #[test]
    fn stale_base_identity_cannot_equal_a_fresh_boot_base() {
        let old = crate::identity::derive_base(&[1; 32], "conduitos/xhci/0000:00:01.0/1b36:000d");
        let fresh = crate::identity::derive_base(&[2; 32], "conduitos/xhci/0000:00:01.0/1b36:000d");
        assert_ne!(old, fresh);
    }

    #[test]
    fn all_refusals_remain_machine_readable() {
        for error in [
            XhciError::Absent,
            XhciError::WrongClass,
            XhciError::InvalidBar,
            XhciError::InvalidLayout,
            XhciError::UnsupportedPageSize,
            XhciError::ScratchpadsUnsupported,
            XhciError::ResetTimeout,
            XhciError::StartTimeout,
            XhciError::CommandRingFull,
            XhciError::UnexpectedCompletion,
            XhciError::CommandTimeout,
            XhciError::DmaAddressInvalid,
        ] {
            assert!(error.as_str().starts_with("xhci-"));
        }
    }
}
