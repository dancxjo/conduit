//! Fixed-capacity xHCI controller mechanism, independent of bus discovery.
//!
//! The native provider supplies mapped registers, exact bus identity, and
//! coherent DMA mapping. It owns device-memory ordering and firmware release.
//! This mechanism contains no USB descriptor or class policy.

use core::{
    hint::spin_loop,
    ptr::{read_volatile, write_volatile},
};

#[path = "xhci_mechanism/event.rs"]
mod event;
#[path = "xhci_mechanism/ordering.rs"]
mod ordering;
pub(crate) use ordering::publish_dma;
pub(crate) mod isochronous;
#[path = "xhci_mechanism/transfer_publication.rs"]
mod transfer_publication;
pub(crate) use transfer_publication::publish_transfer;

const COMMAND_TRBS: usize = 16;
const EVENT_TRBS: usize = 16;
/// Keyboard, pointer, and one USB-backed Line are the complete admitted
/// device set for the pinned ConduitOS Q35 proof.
const ADMITTED_DEVICE_SLOTS: u8 = 3;
const DCBAA_ENTRIES: usize = ADMITTED_DEVICE_SLOTS as usize + 1;
const MAX_PENDING_COMMANDS: u8 = 1;
pub(crate) const POLL_STEPS: u32 = 2_000_000;
const SIGN_SLOTS: u8 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XhciError {
    Absent,
    WrongClass,
    InvalidBar,
    InvalidLayout,
    InsufficientDeviceSlots,
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
            Self::InsufficientDeviceSlots => "xhci-insufficient-device-slots",
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

#[derive(Debug, Eq, PartialEq)]
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
    operational: usize,
    runtime_interrupter: usize,
    doorbell: usize,
    dma_physical: u64,
    command_enqueue: usize,
    command_cycle: u32,
    event_dequeue: usize,
    event_cycle: u32,
    maximum_ports: u8,
    context_bytes: u8,
    isochronous_threshold: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Event {
    pub event_type: u8,
    pub completion_code: u8,
    pub slot: u8,
    pub endpoint: u8,
    pub residual: u32,
    pub pointer: u64,
}

#[repr(C, align(64))]
struct DmaStorage {
    // Entry zero is reserved by xHCI; admitted slots occupy entries 1..=3.
    dcbaa: [u64; DCBAA_ENTRIES],
    _dcbaa_padding: [u8; 32],
    command_ring: [[u32; 4]; COMMAND_TRBS],
    event_ring: [[u32; 4]; EVENT_TRBS],
    erst: [u64; 2],
    _erst_padding: [u8; 48],
}

static mut DMA: DmaStorage = DmaStorage {
    dcbaa: [0; DCBAA_ENTRIES],
    _dcbaa_padding: [0; 32],
    command_ring: [[0; 4]; COMMAND_TRBS],
    event_ring: [[0; 4]; EVENT_TRBS],
    erst: [0; 2],
    _erst_padding: [0; 48],
};

/// Exact native controller coordinates; discovering/configuring them is a
/// platform responsibility, not a consequence of checked Source.
pub(crate) struct ControllerLocation {
    pub segment: u8,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor: u16,
    pub device_id: u16,
    pub bar_physical: u64,
}

/// Initialize the one exclusively owned controller against native mappings.
///
/// # Safety
/// The provider must release firmware ownership and exclusively reserve the
/// controller and this module's fixed DMA storage for their entire lifetime.
/// `mmio` must map the whole register aperture as device memory; the supplied
/// mapping must identify coherent, contiguous controller-addressable DMA.
/// Register writes must order prior DMA publication and event payload loads
/// must follow controller ownership publication. The x86 provider
/// establishes these laws through coherent RAM and x86 device/load ordering.
/// The pinned AArch64 virt leaf uses coherent emulated RAM, Device-nGnRnE
/// registers and full-system DMA publication/acquisition barriers.
/// A new architecture must supply and prove its equivalent ordering/cache
/// leaves before enabling this mechanism; CPU compilation is insufficient.
pub(crate) unsafe fn initialize_mapped(
    location: ControllerLocation,
    mmio: usize,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<XhciReady, XhciError> {
    let dma_virtual = core::ptr::addr_of_mut!(DMA) as u64;
    let dma_physical =
        image_virtual_to_physical(dma_virtual).ok_or(XhciError::DmaAddressInvalid)?;
    if dma_physical & 63 != 0 {
        return Err(XhciError::DmaAddressInvalid);
    }
    let registers = unsafe { initialize_registers(mmio, dma_physical)? };
    Ok(XhciReady {
        segment: location.segment,
        bus: location.bus,
        device: location.device,
        function: location.function,
        vendor: location.vendor,
        device_id: location.device_id,
        bar_physical: location.bar_physical,
        hardware_slots: registers.hardware_slots,
        admitted_slots: ADMITTED_DEVICE_SLOTS,
        command_trbs: COMMAND_TRBS as u8,
        event_trbs: EVENT_TRBS as u8,
        dma_bytes: core::mem::size_of::<DmaStorage>() as u16,
        dma_alignment: core::mem::align_of::<DmaStorage>() as u16,
        maximum_pending_commands: MAX_PENDING_COMMANDS,
        poll_steps: POLL_STEPS,
        sign_slots: SIGN_SLOTS,
        operational: registers.operational,
        runtime_interrupter: registers.runtime_interrupter,
        doorbell: registers.doorbell,
        dma_physical,
        command_enqueue: 1,
        command_cycle: 1,
        event_dequeue: 1,
        event_cycle: 1,
        maximum_ports: registers.maximum_ports,
        context_bytes: registers.context_bytes,
        isochronous_threshold: registers.isochronous_threshold,
    })
}

struct RegisterState {
    hardware_slots: u8,
    maximum_ports: u8,
    context_bytes: u8,
    isochronous_threshold: u8,
    operational: usize,
    runtime_interrupter: usize,
    doorbell: usize,
}

unsafe fn initialize_registers(mmio: usize, dma_physical: u64) -> Result<RegisterState, XhciError> {
    let cap_length = unsafe { read8(mmio) } as usize;
    let hcs1 = unsafe { read32(mmio + 4) };
    let hcs2 = unsafe { read32(mmio + 8) };
    let hardware_slots = (hcs1 & 0xff) as u8;
    let maximum_ports = (hcs1 >> 24) as u8;
    let context_bytes = if unsafe { read32(mmio + 0x10) } & (1 << 2) != 0 {
        64
    } else {
        32
    };
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
    if hardware_slots < ADMITTED_DEVICE_SLOTS {
        return Err(XhciError::InsufficientDeviceSlots);
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
        (*dma).dcbaa = [0; DCBAA_ENTRIES];
    }
    unsafe {
        write64(operational + 0x18, command_phys | 1);
        write64(operational + 0x30, dcbaa_phys);
        write32(operational + 0x38, u32::from(ADMITTED_DEVICE_SLOTS));
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
        transfer_publication::write_command_slot(0, [0, 0, 0, (23 << 10) | 1]);
        write32(doorbell, 0);
    }
    for _ in 0..POLL_STEPS {
        let control = unsafe { read_volatile(core::ptr::addr_of!((*dma).event_ring[0][3])) };
        if control & 1 != 0 {
            ordering::acquire_dma();
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
            return Ok(RegisterState {
                hardware_slots,
                maximum_ports,
                context_bytes,
                operational,
                runtime_interrupter: interrupter,
                isochronous_threshold: (hcs2 & 15) as u8,
                doorbell,
            });
        }
        spin_loop();
    }
    Err(XhciError::CommandTimeout)
}

impl XhciReady {
    pub(crate) fn microframe_index(&self) -> u16 {
        unsafe { (read32(self.runtime_interrupter - 0x20) & 0x3fff) as u16 }
    }
    pub(crate) const fn isochronous_threshold(&self) -> u8 {
        self.isochronous_threshold
    }

    pub(crate) const fn maximum_ports(&self) -> u8 {
        self.maximum_ports
    }

    pub(crate) const fn context_bytes(&self) -> usize {
        self.context_bytes as usize
    }

    pub(crate) fn port_status(&self, port: u8) -> u32 {
        unsafe { read32(self.operational + 0x400 + usize::from(port - 1) * 0x10) }
    }

    pub(crate) fn write_port_status(&self, port: u8, value: u32) {
        unsafe {
            write32(
                self.operational + 0x400 + usize::from(port - 1) * 0x10,
                value,
            )
        }
    }

    pub(crate) fn set_device_context(&self, slot: u8, physical: u64) {
        unsafe {
            write_volatile(
                core::ptr::addr_of_mut!(DMA.dcbaa[usize::from(slot)]),
                physical,
            )
        }
    }

    pub(crate) fn command(&mut self, mut trb: [u32; 4]) -> Result<Event, XhciError> {
        if self.command_enqueue >= COMMAND_TRBS - 1 {
            return Err(XhciError::CommandRingFull);
        }
        trb[3] = (trb[3] & !1) | self.command_cycle;
        let index = self.command_enqueue;
        let pointer = self.dma_physical
            + core::mem::offset_of!(DmaStorage, command_ring) as u64
            + (index * 16) as u64;
        unsafe { transfer_publication::write_command_slot(index, trb) };
        self.command_enqueue += 1;
        unsafe { write32(self.doorbell, 0) };
        for _ in 0..EVENT_TRBS {
            let event = self.next_event()?;
            if event.event_type == 34 {
                continue;
            }
            if event.event_type != 33 || event.pointer != pointer {
                return Err(XhciError::UnexpectedCompletion);
            }
            return Ok(event);
        }
        Err(XhciError::UnexpectedCompletion)
    }

    /// Retires one removed slot while boundedly draining only its stale
    /// transfer completions. Port-change events are controller observations,
    /// not command completions or semantic input.
    pub(crate) fn disable_removed_slot(&mut self, slot: u8) -> Result<u8, XhciError> {
        if slot == 0 || self.command_enqueue >= COMMAND_TRBS - 1 {
            return Err(XhciError::CommandRingFull);
        }
        let index = self.command_enqueue;
        let pointer = self.dma_physical
            + core::mem::offset_of!(DmaStorage, command_ring) as u64
            + (index * 16) as u64;
        let command = [
            0,
            0,
            0,
            (10 << 10) | (u32::from(slot) << 24) | self.command_cycle,
        ];
        unsafe { transfer_publication::write_command_slot(index, command) };
        self.command_enqueue += 1;
        unsafe { write32(self.doorbell, 0) };
        let mut stale_transfers = 0_u8;
        for _ in 0..EVENT_TRBS {
            let event = self.next_event()?;
            match event.event_type {
                34 => continue,
                32 if event.slot == slot => {
                    stale_transfers = stale_transfers
                        .checked_add(1)
                        .ok_or(XhciError::UnexpectedCompletion)?;
                }
                33 if event.pointer == pointer
                    && event.completion_code == 1
                    && event.slot == slot =>
                {
                    unsafe {
                        write_volatile(core::ptr::addr_of_mut!(DMA.dcbaa[usize::from(slot)]), 0)
                    };
                    return Ok(stale_transfers);
                }
                _ => return Err(XhciError::UnexpectedCompletion),
            }
        }
        Err(XhciError::UnexpectedCompletion)
    }

    pub(crate) fn ring_endpoint(&self, slot: u8, endpoint: u8) {
        unsafe { write32(self.doorbell + usize::from(slot) * 4, u32::from(endpoint)) }
    }

    pub(crate) fn next_event(&mut self) -> Result<Event, XhciError> {
        for _ in 0..POLL_STEPS {
            if let Some(event) = self.poll_event() {
                return Ok(event);
            }
            spin_loop();
        }
        Err(XhciError::CommandTimeout)
    }
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

unsafe fn read8(address: usize) -> u8 {
    unsafe { read_volatile(address as *const u8) }
}
unsafe fn read32(address: usize) -> u32 {
    unsafe { read_volatile(address as *const u32) }
}
unsafe fn write32(address: usize, value: u32) {
    ordering::publish_dma();
    unsafe { write_volatile(address as *mut u32, value) };
    ordering::complete_device_write();
}
unsafe fn write64(address: usize, value: u64) {
    ordering::publish_dma();
    unsafe { write_volatile(address as *mut u64, value) };
    ordering::complete_device_write();
}

#[cfg(test)]
#[path = "xhci_mechanism/tests.rs"]
mod tests;

/// Serialized synthetic controller memory. Exercises production ring consumption
/// and Disable Slot publication; it does not emulate a hardware controller.
#[cfg(test)]
pub(crate) fn with_synthetic_stop_controller(
    event_slot: u8,
    completion_code: u8,
    pointer_delta: u64,
    test: impl FnOnce(XhciReady),
) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().unwrap();
    let mut registers = [0_u64; 32];
    // SAFETY: every field is an integer; zero is valid. The fixture owns the
    // synthetic registers and serializes the sole global controller DMA.
    let mut controller: XhciReady = unsafe { core::mem::zeroed() };
    controller.operational = registers.as_mut_ptr() as usize;
    controller.runtime_interrupter = controller.operational;
    controller.doorbell = controller.operational + 128;
    controller.dma_physical = 0x1000;
    controller.command_cycle = 1;
    controller.event_cycle = 1;
    controller.maximum_ports = 1;
    controller.admitted_slots = 3;
    let pointer = controller.dma_physical
        + core::mem::offset_of!(DmaStorage, command_ring) as u64
        + pointer_delta;
    // SAFETY: fixture lock excludes other synthetic global DMA users.
    unsafe {
        DMA.event_ring = [[0; 4]; EVENT_TRBS];
        DMA.command_ring = [[0; 4]; COMMAND_TRBS];
        DMA.dcbaa[1] = 0xfeed0000;
        DMA.event_ring[0] = [
            pointer as u32,
            (pointer >> 32) as u32,
            u32::from(completion_code) << 24,
            (33 << 10) | (u32::from(event_slot) << 24) | 1,
        ];
    }
    test(controller);
}

#[cfg(test)]
pub(crate) unsafe fn synthetic_stop_dcbaa() -> u64 {
    // SAFETY: caller holds with_synthetic_stop_controller's fixture lock.
    unsafe { read_volatile(core::ptr::addr_of!(DMA.dcbaa[1])) }
}

#[cfg(test)]
pub(crate) unsafe fn synthetic_late_original_stop_ack(controller: &XhciReady) {
    let pointer = controller.dma_physical + core::mem::offset_of!(DmaStorage, command_ring) as u64;
    // SAFETY: caller holds the serialized fixture lock, and the first event
    // has been consumed. This completion names the original command, not retry.
    unsafe {
        DMA.event_ring[controller.event_dequeue] = [
            pointer as u32,
            (pointer >> 32) as u32,
            1 << 24,
            (33 << 10) | (1 << 24) | controller.event_cycle,
        ];
    }
}
