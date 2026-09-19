//! Fixed-storage transitional VirtIO-net link device for the QEMU x86 Image.
//!
//! This is raw Ethernet authority only. IP, TCP, TLS, WebSocket, and Line
//! meaning remain separate admitted layers.

use core::{
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use super::{
    io::{inb, inl, inw, outb, outl, outw},
    virtio_net_pci,
};

const DEVICE_FEATURES: u16 = 0x00;
const GUEST_FEATURES: u16 = 0x04;
const QUEUE_ADDRESS: u16 = 0x08;
const QUEUE_SIZE: u16 = 0x0c;
const QUEUE_SELECT: u16 = 0x0e;
const QUEUE_NOTIFY: u16 = 0x10;
const DEVICE_STATUS: u16 = 0x12;
const DEVICE_CONFIG: u16 = 0x14;
const FEATURE_MAC: u32 = 1 << 5;
const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FAILED: u8 = 128;
const RECEIVE_QUEUE: u16 = 0;
const TRANSMIT_QUEUE: u16 = 1;
const DESCRIPTOR_WRITE: u16 = 2;
// QEMU's selected transitional device exposes exactly 256 entries. The split
// ring therefore occupies three contiguous pages: descriptors and available
// ring in the first two, used ring beginning at the third page.
const RING_ENTRIES: u16 = 256;
const AVAILABLE_OFFSET: usize = 4096;
const USED_OFFSET: usize = 8192;
const QUEUE_BYTES: usize = 12_288;
const FRAME_BYTES: usize = 2048;
const LEGACY_HEADER_BYTES: usize = 10;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

#[repr(C, align(4096))]
struct QueueMemory([u8; QUEUE_BYTES]);

#[repr(C, align(4096))]
struct FrameMemory([u8; FRAME_BYTES]);

static mut RECEIVE_RING: QueueMemory = QueueMemory([0; QUEUE_BYTES]);
static mut TRANSMIT_RING: QueueMemory = QueueMemory([0; QUEUE_BYTES]);
static mut RECEIVE_FRAME: FrameMemory = FrameMemory([0; FRAME_BYTES]);
static mut TRANSMIT_FRAME: FrameMemory = FrameMemory([0; FRAME_BYTES]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioNetError {
    Absent,
    WrongDevice,
    InvalidBar,
    InvalidIdentity,
    AlreadyInitialized,
    FeatureMissing,
    QueueUnavailable,
    QueueSizeUnsupported,
    DmaAddressInvalid,
    DmaNotContiguous,
    DeviceFailed,
    Pressure,
    Timeout,
    MalformedCompletion,
    FrameTooLarge,
    ReceiveBufferTooSmall,
}

impl VirtioNetError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "virtio-net-absent",
            Self::WrongDevice => "virtio-net-wrong-device",
            Self::InvalidBar => "virtio-net-invalid-bar",
            Self::InvalidIdentity => "virtio-net-identity-invalid",
            Self::AlreadyInitialized => "virtio-net-already-initialized",
            Self::FeatureMissing => "virtio-net-feature-missing",
            Self::QueueUnavailable => "virtio-net-queue-unavailable",
            Self::QueueSizeUnsupported => "virtio-net-queue-size-unsupported",
            Self::DmaAddressInvalid => "virtio-net-dma-address-invalid",
            Self::DmaNotContiguous => "virtio-net-dma-not-contiguous",
            Self::DeviceFailed => "virtio-net-device-failed",
            Self::Pressure => "virtio-net-pressure",
            Self::Timeout => "virtio-net-timeout",
            Self::MalformedCompletion => "virtio-net-completion-malformed",
            Self::FrameTooLarge => "virtio-net-frame-too-large",
            Self::ReceiveBufferTooSmall => "virtio-net-receive-buffer-too-small",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioNetIdentity {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub mac: [u8; 6],
    pub provider_generation: u64,
    pub boot_id: [u8; 32],
}

pub struct VirtioNetReady {
    identity: VirtioNetIdentity,
    io_base: u16,
    receive: LegacyQueue,
    transmit: LegacyQueue,
}

impl VirtioNetReady {
    pub const fn identity(&self) -> VirtioNetIdentity {
        self.identity
    }

    pub fn send(&mut self, frame: &[u8], maximum_polls: u32) -> Result<(), VirtioNetError> {
        self.check_device()?;
        if frame.is_empty() || frame.len() > FRAME_BYTES - LEGACY_HEADER_BYTES {
            return Err(VirtioNetError::FrameTooLarge);
        }
        if self.transmit.pending {
            return Err(VirtioNetError::Pressure);
        }
        if maximum_polls == 0 {
            return Err(VirtioNetError::Timeout);
        }
        unsafe {
            let buffer = ptr::addr_of_mut!(TRANSMIT_FRAME.0).cast::<u8>();
            ptr::write_bytes(buffer, 0, LEGACY_HEADER_BYTES);
            ptr::copy_nonoverlapping(frame.as_ptr(), buffer.add(LEGACY_HEADER_BYTES), frame.len());
        }
        self.transmit
            .publish(0, LEGACY_HEADER_BYTES + frame.len(), false)?;
        unsafe { outw(self.io_base + QUEUE_NOTIFY, TRANSMIT_QUEUE) };
        // A transmit chain has no device-write descriptor, so its used length
        // is allowed to be zero. Exact descriptor identity is still checked.
        self.transmit.wait_for_completion(maximum_polls)?;
        unsafe {
            ptr::write_bytes(
                ptr::addr_of_mut!(TRANSMIT_FRAME.0).cast::<u8>(),
                0,
                FRAME_BYTES,
            )
        };
        Ok(())
    }

    pub fn receive(&mut self, output: &mut [u8]) -> Result<usize, VirtioNetError> {
        self.check_device()?;
        let Some(completion) = self.receive.take_completion()? else {
            return Err(VirtioNetError::Pressure);
        };
        let payload = completion
            .checked_sub(LEGACY_HEADER_BYTES)
            .ok_or(VirtioNetError::MalformedCompletion)?;
        if payload > FRAME_BYTES - LEGACY_HEADER_BYTES {
            return Err(VirtioNetError::MalformedCompletion);
        }
        if payload > output.len() {
            self.repost_receive()?;
            return Err(VirtioNetError::ReceiveBufferTooSmall);
        }
        unsafe {
            let buffer = ptr::addr_of!(RECEIVE_FRAME.0).cast::<u8>();
            ptr::copy_nonoverlapping(
                buffer.add(LEGACY_HEADER_BYTES),
                output.as_mut_ptr(),
                payload,
            );
        }
        self.repost_receive()?;
        Ok(payload)
    }

    fn repost_receive(&mut self) -> Result<(), VirtioNetError> {
        unsafe {
            ptr::write_bytes(
                ptr::addr_of_mut!(RECEIVE_FRAME.0).cast::<u8>(),
                0,
                FRAME_BYTES,
            )
        };
        self.receive.publish(0, FRAME_BYTES, true)?;
        unsafe { outw(self.io_base + QUEUE_NOTIFY, RECEIVE_QUEUE) };
        Ok(())
    }

    fn check_device(&self) -> Result<(), VirtioNetError> {
        let status = unsafe { inb(self.io_base + DEVICE_STATUS) };
        if status & STATUS_FAILED != 0 || status & STATUS_DRIVER_OK == 0 {
            Err(VirtioNetError::DeviceFailed)
        } else {
            Ok(())
        }
    }
}

pub fn initialize_virtio_net(
    boot_id: [u8; 32],
    provider_generation: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<VirtioNetReady, VirtioNetError> {
    if boot_id == [0; 32] || provider_generation == 0 {
        return Err(VirtioNetError::InvalidIdentity);
    }
    if INITIALIZED.swap(true, Ordering::AcqRel) {
        return Err(VirtioNetError::AlreadyInitialized);
    }
    let result = initialize_inner(boot_id, provider_generation, image_virtual_to_physical);
    if result.is_err() {
        INITIALIZED.store(false, Ordering::Release);
    }
    result
}

fn initialize_inner(
    boot_id: [u8; 32],
    provider_generation: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<VirtioNetReady, VirtioNetError> {
    let pci = virtio_net_pci::discover()?;
    unsafe {
        outb(pci.io_base + DEVICE_STATUS, 0);
        outb(
            pci.io_base + DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER,
        );
    }
    let features = unsafe { inl(pci.io_base + DEVICE_FEATURES) };
    if features & FEATURE_MAC == 0 {
        return fail(pci.io_base, VirtioNetError::FeatureMissing);
    }
    unsafe { outl(pci.io_base + GUEST_FEATURES, FEATURE_MAC) };

    let receive = prepare_queue(
        pci.io_base,
        RECEIVE_QUEUE,
        ptr::addr_of_mut!(RECEIVE_RING),
        image_virtual_to_physical,
    )?;
    let transmit = prepare_queue(
        pci.io_base,
        TRANSMIT_QUEUE,
        ptr::addr_of_mut!(TRANSMIT_RING),
        image_virtual_to_physical,
    )?;
    let receive_frame_physical = physical_region(
        ptr::addr_of!(RECEIVE_FRAME) as u64,
        FRAME_BYTES,
        image_virtual_to_physical,
    )?;
    let transmit_frame_physical = physical_region(
        ptr::addr_of!(TRANSMIT_FRAME) as u64,
        FRAME_BYTES,
        image_virtual_to_physical,
    )?;
    let mut ready = VirtioNetReady {
        identity: VirtioNetIdentity {
            bus: pci.bus,
            device: pci.device,
            function: pci.function,
            mac: core::array::from_fn(|index| unsafe {
                inb(pci.io_base + DEVICE_CONFIG + index as u16)
            }),
            provider_generation,
            boot_id,
        },
        io_base: pci.io_base,
        receive,
        transmit,
    };
    ready
        .receive
        .set_descriptor_address(0, receive_frame_physical);
    ready
        .transmit
        .set_descriptor_address(0, transmit_frame_physical);
    unsafe {
        outb(
            pci.io_base + DEVICE_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK,
        );
    }
    ready.check_device()?;
    ready.repost_receive()?;
    Ok(ready)
}

fn prepare_queue(
    io_base: u16,
    queue: u16,
    memory: *mut QueueMemory,
    translate: fn(u64) -> Option<u64>,
) -> Result<LegacyQueue, VirtioNetError> {
    unsafe { outw(io_base + QUEUE_SELECT, queue) };
    let size = unsafe { inw(io_base + QUEUE_SIZE) };
    if size == 0 {
        return fail(io_base, VirtioNetError::QueueUnavailable);
    }
    if size != RING_ENTRIES {
        return fail(io_base, VirtioNetError::QueueSizeUnsupported);
    }
    let physical = physical_region(memory as u64, QUEUE_BYTES, translate)?;
    let page_frame =
        u32::try_from(physical >> 12).map_err(|_| VirtioNetError::DmaAddressInvalid)?;
    unsafe {
        ptr::write_bytes(memory.cast::<u8>(), 0, QUEUE_BYTES);
        outl(io_base + QUEUE_ADDRESS, page_frame);
    }
    Ok(LegacyQueue::new(memory.cast::<u8>()))
}

fn physical_region(
    virtual_address: u64,
    bytes: usize,
    translate: fn(u64) -> Option<u64>,
) -> Result<u64, VirtioNetError> {
    let first = translate(virtual_address).ok_or(VirtioNetError::DmaAddressInvalid)?;
    if first & 0xfff != 0 {
        return Err(VirtioNetError::DmaAddressInvalid);
    }
    for offset in (4096..bytes).step_by(4096) {
        if translate(virtual_address + offset as u64) != Some(first + offset as u64) {
            return Err(VirtioNetError::DmaNotContiguous);
        }
    }
    Ok(first)
}

fn fail<T>(io_base: u16, error: VirtioNetError) -> Result<T, VirtioNetError> {
    unsafe { outb(io_base + DEVICE_STATUS, STATUS_FAILED) };
    Err(error)
}

struct LegacyQueue {
    memory: *mut u8,
    available: u16,
    consumed: u16,
    pending: bool,
}

impl LegacyQueue {
    const fn new(memory: *mut u8) -> Self {
        Self {
            memory,
            available: 0,
            consumed: 0,
            pending: false,
        }
    }

    fn set_descriptor_address(&mut self, descriptor: u16, physical: u64) {
        unsafe {
            ptr::write_volatile(
                self.memory.add(usize::from(descriptor) * 16).cast(),
                physical,
            )
        };
    }

    fn publish(
        &mut self,
        descriptor: u16,
        len: usize,
        device_writes: bool,
    ) -> Result<(), VirtioNetError> {
        if self.pending {
            return Err(VirtioNetError::Pressure);
        }
        let descriptor_offset = usize::from(descriptor) * 16;
        unsafe {
            ptr::write_volatile(
                self.memory.add(descriptor_offset + 8).cast::<u32>(),
                len as u32,
            );
            ptr::write_volatile(
                self.memory.add(descriptor_offset + 12).cast::<u16>(),
                if device_writes { DESCRIPTOR_WRITE } else { 0 },
            );
            let ring_offset = AVAILABLE_OFFSET + 4 + usize::from(self.available % RING_ENTRIES) * 2;
            ptr::write_volatile(self.memory.add(ring_offset).cast::<u16>(), descriptor);
            self.available = self.available.wrapping_add(1);
            core::sync::atomic::fence(Ordering::Release);
            ptr::write_volatile(
                self.memory.add(AVAILABLE_OFFSET + 2).cast::<u16>(),
                self.available,
            );
        }
        self.pending = true;
        Ok(())
    }

    fn take_completion(&mut self) -> Result<Option<usize>, VirtioNetError> {
        core::sync::atomic::fence(Ordering::Acquire);
        let used = unsafe { ptr::read_volatile(self.memory.add(USED_OFFSET + 2).cast::<u16>()) };
        if used == self.consumed {
            return Ok(None);
        }
        if used != self.consumed.wrapping_add(1) || !self.pending {
            return Err(VirtioNetError::MalformedCompletion);
        }
        let slot = usize::from(self.consumed % RING_ENTRIES);
        let id = unsafe {
            ptr::read_volatile(self.memory.add(USED_OFFSET + 4 + slot * 8).cast::<u32>())
        };
        let len = unsafe {
            ptr::read_volatile(self.memory.add(USED_OFFSET + 8 + slot * 8).cast::<u32>())
        };
        if id != 0 {
            return Err(VirtioNetError::MalformedCompletion);
        }
        self.consumed = used;
        self.pending = false;
        Ok(Some(len as usize))
    }

    fn wait_for_completion(&mut self, maximum_polls: u32) -> Result<usize, VirtioNetError> {
        for _ in 0..maximum_polls {
            if let Some(len) = self.take_completion()? {
                return Ok(len);
            }
            core::hint::spin_loop();
        }
        Err(VirtioNetError::Timeout)
    }
}

#[cfg(test)]
#[path = "virtio_net_tests.rs"]
mod tests;
