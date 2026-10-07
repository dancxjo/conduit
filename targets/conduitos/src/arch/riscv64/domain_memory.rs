//! Sv39 user mappings beneath supervisor-only inherited Root subtrees.
use super::ordinary_domain::TextFrame;
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
const PPN: u64 = (1 << 44) - 1;
const U: u64 = 1 << 4;
const RESERVED: u64 = !((1_u64 << 54) - 1);
pub(super) const USER_FRAME: u64 = USER_TEXT_START + MAXIMUM_IMAGE_BYTES;
pub(super) const USER_STACK_TOP: u64 = USER_TEXT_START + 0x24000;
static HHDM: AtomicU64 = AtomicU64::new(0);
static OWNED: AtomicU8 = AtomicU8::new(0);

#[repr(C, align(4096))]
struct Table([u64; 512]);
#[repr(C, align(4096))]
struct Bytes<const N: usize>([u8; N]);
#[repr(C, align(16))]
struct Floating([u8; 272]);
#[repr(C)]
struct Slot {
    root: Table,
    user_l1: Table,
    user_l0: Table,
    code: Bytes<65536>,
    frame: Bytes<4096>,
    stack: Bytes<16384>,
    trap: Bytes<16384>,
    root_floating: Floating,
    user_floating: Floating,
}
impl Slot {
    const fn empty() -> Self {
        Self {
            root: Table([0; 512]),
            user_l1: Table([0; 512]),
            user_l0: Table([0; 512]),
            code: Bytes([0; 65536]),
            frame: Bytes([0; 4096]),
            stack: Bytes([0; 16384]),
            trap: Bytes([0; 16384]),
            root_floating: Floating([0; 272]),
            user_floating: Floating([0; 272]),
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
    pub satp: u64,
}
impl AddressSpace {
    pub const RESERVED_BYTES: u32 = core::mem::size_of::<Slot>() as u32;
    pub fn install(image: &DomainImage<'_>) -> Result<Self, DomainRefusal> {
        enable_no_execute()?;
        let old: u64;
        unsafe {
            core::arch::asm!("csrr {0}, satp", out(reg) old, options(nostack));
        }
        let root = physical_mapping((old & PPN) << 12)? as *const u64;
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
            satp: 0,
        };
        let memory = unsafe { &mut (*POOL.0.get())[slot] };
        // Sv39 has no hierarchical user-deny bit. Earn the inherited boundary
        // by checking every reachable high-half leaf before sharing its tables.
        // The depth and total inspected descriptors are both bounded.
        let mut remaining = 32768_u32;
        for index in 256..512 {
            let descriptor = unsafe { root.add(index).read_volatile() };
            validate_root(descriptor, 2, &mut remaining)?;
            memory.root.0[index] = descriptor;
        }
        memory.root.0[0] = table_descriptor(&memory.user_l1)?;
        memory.user_l1.0[(USER_TEXT_START >> 21) as usize] = table_descriptor(&memory.user_l0)?;
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
            let mapped = offset
                .checked_add(segment.mapped_bytes() as usize)
                .filter(|end| *end <= memory.code.0.len())
                .ok_or(DomainRefusal::InvalidMemory)?;
            for page in (offset..mapped).step_by(PAGE) {
                memory.user_l0.0[page / PAGE] = leaf(
                    physical(&memory.code)? + page as u64,
                    2 | if segment.executable { 8 } else { 0 },
                );
            }
        }
        memory.user_l0.0[16] = leaf(physical(&memory.frame)?, 2 | 4);
        for page in 0..4 {
            memory.user_l0.0[32 + page] =
                leaf(physical(&memory.stack)? + (page * PAGE) as u64, 2 | 4);
        }
        space.satp = (8 << 60) | (physical(&memory.root)? >> 12);
        unsafe {
            core::arch::asm!("fence rw, rw", "fence.i", options(nostack));
        }
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
fn leaf(address: u64, permissions: u64) -> u64 {
    ((address >> 12) << 10) | 1 | U | (1 << 6) | (1 << 7) | permissions
}
fn table_descriptor<T>(value: &T) -> Result<u64, DomainRefusal> {
    Ok(((physical(value)? >> 12) << 10) | 1)
}
fn physical<T>(value: &T) -> Result<u64, DomainRefusal> {
    boot::executable_physical_address(value as *const T as u64)
        .filter(|address| *address != 0 && address & 0xfff == 0 && *address >> 56 == 0)
        .ok_or(DomainRefusal::InvalidMemory)
}
fn physical_mapping(address: u64) -> Result<u64, DomainRefusal> {
    let hhdm = HHDM.load(Ordering::Acquire);
    if hhdm == 0 || address == 0 {
        return Err(DomainRefusal::InvalidMemory);
    }
    hhdm.checked_add(address)
        .ok_or(DomainRefusal::InvalidMemory)
}
fn validate_root(entry: u64, level: u32, remaining: &mut u32) -> Result<(), DomainRefusal> {
    *remaining = remaining.checked_sub(1).ok_or(DomainRefusal::Unsupported)?;
    if entry & 1 == 0 {
        return Ok(());
    }
    if entry & (U | RESERVED) != 0 || entry & 6 == 4 {
        return Err(DomainRefusal::Unsupported);
    }
    let physical = ((entry >> 10) & PPN) << 12;
    if entry & 0xe != 0 {
        if physical & ((1 << (12 + 9 * level)) - 1) != 0 {
            return Err(DomainRefusal::Unsupported);
        }
        return Ok(());
    }
    if level == 0 || entry & 0xc0 != 0 {
        return Err(DomainRefusal::Unsupported);
    }
    let table = physical_mapping(physical)? as *const u64;
    for index in 0..512 {
        validate_root(
            unsafe { table.add(index).read_volatile() },
            level - 1,
            remaining,
        )?;
    }
    Ok(())
}
pub(super) fn enable_no_execute() -> Result<(), DomainRefusal> {
    let (satp, status): (u64, u64);
    unsafe {
        core::arch::asm!("csrr {0}, satp", "csrr {1}, sstatus", out(reg) satp, out(reg) status, options(nostack));
    }
    if satp >> 60 != 8 || (status >> 32) & 3 != 2 || status & (1 << 6) != 0 {
        Err(DomainRefusal::Unsupported)
    } else {
        Ok(())
    }
}
