//! Bounded EL0 mappings; inherited high Root mappings remain privileged.
use super::ordinary_domain::TextFrame;
use crate::domain_layout::{RETAINED_BYTES, RETAINED_PAGE, STACK_BYTES, STACK_PAGE};
use crate::{
    boot,
    domain_image::{DomainImage, MAXIMUM_IMAGE_BYTES, USER_TEXT_START},
    protected_region::DomainRefusal,
};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
};

const PAGE: usize = 4096;
const ADDRESS: u64 = 0x0000_ffff_ffff_f000;
const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;
const ROOT_ONLY_TABLE: u64 = (1 << 61) | (1 << 60);
pub(super) const USER_FRAME: u64 = USER_TEXT_START + MAXIMUM_IMAGE_BYTES;
pub(super) const USER_STACK_TOP: u64 = USER_TEXT_START + 0x48000;
static HHDM: AtomicU64 = AtomicU64::new(0);
static OWNED: AtomicU8 = AtomicU8::new(0);

#[repr(C, align(4096))]
struct Table([u64; 512]);
#[repr(C, align(4096))]
struct Bytes<const N: usize>([u8; N]);
#[repr(C, align(16))]
struct FloatingState([u8; 528]);
#[repr(C)]
struct Slot {
    low_l0: Table,
    low_l1: Table,
    low_l2: Table,
    user_l3: Table,
    root_l0: Table,
    code: Bytes<131072>,
    frame: Bytes<4096>,
    stack: Bytes<STACK_BYTES>,
    retained: Bytes<RETAINED_BYTES>,
    trap: Bytes<16384>,
    root_floating: FloatingState,
    user_floating: FloatingState,
}
impl Slot {
    const fn empty() -> Self {
        Self {
            low_l0: Table([0; 512]),
            low_l1: Table([0; 512]),
            low_l2: Table([0; 512]),
            user_l3: Table([0; 512]),
            root_l0: Table([0; 512]),
            code: Bytes([0; 131072]),
            frame: Bytes([0; 4096]),
            stack: Bytes([0; STACK_BYTES]),
            retained: Bytes([0; RETAINED_BYTES]),
            trap: Bytes([0; 16384]),
            root_floating: FloatingState([0; 528]),
            user_floating: FloatingState([0; 528]),
        }
    }
}
struct Pool(UnsafeCell<[Slot; 2]>);
unsafe impl Sync for Pool {}
static POOL: Pool = Pool(UnsafeCell::new([Slot::empty(), Slot::empty()]));

pub(super) fn initialize(record: &boot::BootRecord) {
    HHDM.store(record.hhdm_offset, Ordering::Release);
}

pub(super) struct AddressSpace {
    slot: usize,
    cleared: bool,
    pub entry: u64,
    pub ttbr0: u64,
    pub ttbr1: u64,
    pub tcr: u64,
}
impl AddressSpace {
    pub const RESERVED_BYTES: u32 = core::mem::size_of::<Slot>() as u32;

    pub fn install(image: &DomainImage<'_>) -> Result<Self, DomainRefusal> {
        let (tcr, root, mair, control): (u64, u64, u64, u64);
        unsafe {
            core::arch::asm!(
                "mrs {0}, tcr_el1", "mrs {1}, ttbr1_el1",
                "mrs {2}, mair_el1", "mrs {3}, sctlr_el1",
                out(reg) tcr, out(reg) root, out(reg) mair, out(reg) control,
                options(nostack, preserves_flags)
            );
        }
        // This backend owns a 4 KiB / 48-bit stage-1 regime. It does not
        // reinterpret an inherited 16/64 KiB, LPA2, or disabled-MMU regime.
        if super::current_el() != 1
            || control & 1 == 0
            || (tcr >> 16) & 0x3f != 16
            || (tcr >> 30) & 3 != 2
            || (tcr >> 32) & 7 > 5
            || tcr & (1 << 23) != 0
            || tcr & (1 << 59) != 0
            || root & ADDRESS == 0
            || HHDM.load(Ordering::Acquire) == 0
        {
            return Err(DomainRefusal::Unsupported);
        }
        let normal_index = (0..8)
            .find(|index| (mair >> (index * 8)) & 0xff == 0xff)
            .ok_or(DomainRefusal::Unsupported)?;
        // The existing Root MMIO setup owns MAIR slot 7 as Device-nGnRnE.
        if mair >> 56 != 0 {
            return Err(DomainRefusal::Unsupported);
        }
        let slot = (0..2)
            .find(|index| {
                let bit = 1 << index;
                OWNED.fetch_or(bit, Ordering::AcqRel) & bit == 0
            })
            .ok_or(DomainRefusal::InvalidMemory)?;
        let mut space = Self {
            slot,
            cleared: false,
            entry: image.entry,
            ttbr0: 0,
            ttbr1: 0,
            tcr: 0,
        };
        let memory = unsafe { &mut (*POOL.0.get())[slot] };
        let root_virtual = HHDM
            .load(Ordering::Acquire)
            .checked_add(root & ADDRESS)
            .ok_or(DomainRefusal::InvalidMemory)?;
        for index in 0..512 {
            let descriptor = unsafe { (root_virtual as *const u64).add(index).read_volatile() };
            if descriptor & 1 != 0 && descriptor & 3 != 3 {
                return Err(DomainRefusal::Unsupported);
            }
            // APTable[0] denies EL0 data access and UXNTable denies EL0
            // instruction fetch throughout every inherited high subtree.
            memory.root_l0.0[index] = if descriptor & 1 == 0 {
                0
            } else {
                descriptor | ROOT_ONLY_TABLE
            };
        }
        memory.low_l0.0[0] = physical(&memory.low_l1)? | 3;
        memory.low_l1.0[0] = physical(&memory.low_l2)? | 3;
        memory.low_l2.0[(USER_TEXT_START >> 21) as usize] = physical(&memory.user_l3)? | 3;
        for address in [super::GICD_BASE, super::UART_BASE] {
            let block = address as u64 & !((1 << 21) - 1);
            memory.low_l2.0[address >> 21] = block | 1 | (1 << 10) | (7 << 2) | PXN | UXN;
        }
        let common = 3 | (normal_index << 2) | (3 << 8) | (1 << 10) | PXN;
        for segment in &image.segments {
            let offset = segment
                .address
                .checked_sub(USER_TEXT_START)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or(DomainRefusal::InvalidMemory)?;
            let end = offset
                .checked_add(segment.bytes.len())
                .ok_or(DomainRefusal::InvalidMemory)?;
            memory
                .code
                .0
                .get_mut(offset..end)
                .ok_or(DomainRefusal::InvalidMemory)?
                .copy_from_slice(segment.bytes);
            let mapped_end = offset
                .checked_add(segment.mapped_bytes() as usize)
                .filter(|end| *end <= memory.code.0.len())
                .ok_or(DomainRefusal::InvalidMemory)?;
            for page in (offset..mapped_end).step_by(PAGE) {
                memory.user_l3.0[page / PAGE] = physical(&memory.code)? + page as u64
                    | common
                    | (3 << 6)
                    | if segment.executable { 0 } else { UXN };
            }
        }
        memory.user_l3.0[32] = physical(&memory.frame)? | common | (1 << 6) | UXN;
        for page in 0..STACK_BYTES / PAGE {
            memory.user_l3.0[STACK_PAGE + page] =
                physical(&memory.stack)? + (page * PAGE) as u64 | common | (1 << 6) | UXN;
        }
        for page in 0..RETAINED_BYTES / PAGE {
            memory.user_l3.0[RETAINED_PAGE + page] =
                physical(&memory.retained)? + (page * PAGE) as u64 | common | (1 << 6) | UXN;
        }
        space.ttbr0 = physical(&memory.low_l0)?;
        space.ttbr1 = physical(&memory.root_l0)?;
        // Enable hierarchical permissions for both halves; do not allow an
        // inherited HPD setting to discard the high Root subtree restriction.
        space.tcr =
            (tcr & !0xffff & !(1 << 37) & !(1 << 39) & !(1 << 40) & !(1 << 41) & !(1 << 42))
                | 16
                | (1 << 8)
                | (1 << 10)
                | (3 << 12);
        synchronize_code(&memory.code.0);
        Ok(space)
    }
    pub fn frame(&mut self) -> &mut TextFrame {
        unsafe {
            &mut *(*POOL.0.get())[self.slot]
                .frame
                .0
                .as_mut_ptr()
                .cast::<TextFrame>()
        }
    }
    pub fn trap_stack_top(&self) -> u64 {
        unsafe { core::ptr::addr_of!((*POOL.0.get())[self.slot].trap) as u64 + 16384 }
    }
    pub fn floating_states(&self) -> (u64, u64) {
        unsafe {
            let slot = &(*POOL.0.get())[self.slot];
            (
                core::ptr::addr_of!(slot.root_floating) as u64,
                core::ptr::addr_of!(slot.user_floating) as u64,
            )
        }
    }
    pub fn quarantine(&mut self) -> u32 {
        if self.cleared {
            return 0;
        }
        unsafe {
            core::ptr::write_bytes(core::ptr::addr_of_mut!((*POOL.0.get())[self.slot]), 0, 1);
        }
        self.cleared = true;
        Self::RESERVED_BYTES
    }
}
impl Drop for AddressSpace {
    fn drop(&mut self) {
        self.quarantine();
        OWNED.fetch_and(!(1 << self.slot), Ordering::Release);
    }
}
fn physical<T>(value: &T) -> Result<u64, DomainRefusal> {
    boot::executable_physical_address(value as *const T as u64)
        .filter(|address| *address != 0 && *address & !ADDRESS == 0)
        .ok_or(DomainRefusal::InvalidMemory)
}
fn synchronize_code(code: &[u8]) {
    let ctr: u64;
    unsafe {
        core::arch::asm!("mrs {0}, ctr_el0", out(reg) ctr, options(nostack));
    }
    let line = 4usize << ((ctr >> 16) & 15);
    for offset in (0..code.len()).step_by(line) {
        unsafe {
            core::arch::asm!("dc cvau, {0}", in(reg) code.as_ptr().add(offset), options(nostack));
        }
    }
    unsafe {
        core::arch::asm!("dsb ish", "ic iallu", "dsb ish", "isb", options(nostack));
    }
}

pub(super) fn enable_no_execute() -> Result<(), DomainRefusal> {
    if super::current_el() == 1 {
        Ok(())
    } else {
        Err(DomainRefusal::Unsupported)
    }
}
