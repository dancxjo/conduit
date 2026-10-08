//! Finite PLV3 mappings in the reviewed Limine 4 KiB paging regime.
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
const ADDRESS: u64 = 0x0000_ffff_ffff_f000;
const NX: u64 = 1 << 62;
const USER: u64 = 3 << 2;
const PWCL: usize = 12 | (9 << 5) | (21 << 10) | (9 << 15) | (30 << 20) | (9 << 25);
const PWCH: usize = 39 | (9 << 6);
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
    low_l3: Table,
    low_l2: Table,
    low_l1: Table,
    user_l0: Table,
    high_l3: Table,
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
            low_l3: Table([0; 512]),
            low_l2: Table([0; 512]),
            low_l1: Table([0; 512]),
            user_l0: Table([0; 512]),
            high_l3: Table([0; 512]),
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
// Slot possession is exclusive; transitions admit only one active CPU entry.
unsafe impl Sync for Pool {}
static POOL: Pool = Pool(UnsafeCell::new([Slot::empty(), Slot::empty()]));
pub(super) fn initialize(record: &boot::BootRecord) {
    HHDM.store(record.hhdm_offset, Ordering::Release);
}

pub(super) struct AddressSpace {
    slot: usize,
    cleared: bool,
    pub entry: u64,
    pub pgdl: u64,
    pub pgdh: u64,
}
impl AddressSpace {
    pub const RESERVED_BYTES: u32 = core::mem::size_of::<Slot>() as u32;
    pub fn install(image: &DomainImage<'_>) -> Result<Self, DomainRefusal> {
        enable_no_execute()?;
        let root = physical_mapping(super::read_csr::<0x1a>() as u64)? as *const u64;
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
            pgdl: 0,
            pgdh: 0,
        };
        let memory = unsafe { &mut (*POOL.0.get())[slot] };
        // LoongArch has no hierarchical PLV deny bit. Inspect every inherited
        // high-half leaf, with bounded depth and a finite descriptor budget.
        let mut remaining = 32768_u32;
        for index in 0..512 {
            let entry = unsafe { root.add(index).read_volatile() };
            validate_root(entry, 3, &mut remaining)?;
            memory.high_l3.0[index] = entry;
        }
        memory.low_l3.0[0] = physical(&memory.low_l2)?;
        memory.low_l2.0[0] = physical(&memory.low_l1)?;
        memory.low_l1.0[(USER_TEXT_START >> 21) as usize] = physical(&memory.user_l0)?;
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
                    if segment.executable { 0 } else { NX },
                );
            }
        }
        memory.user_l0.0[16] = leaf(physical(&memory.frame)?, NX | 2 | (1 << 8));
        for page in 0..4 {
            memory.user_l0.0[32 + page] = leaf(
                physical(&memory.stack)? + (page * PAGE) as u64,
                NX | 2 | (1 << 8),
            );
        }
        space.pgdl = physical(&memory.low_l3)?;
        space.pgdh = physical(&memory.high_l3)?;
        unsafe {
            core::arch::asm!("dbar 0", "ibar 0", options(nostack));
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
    pub fn code_address(&self) -> u64 {
        unsafe { core::ptr::addr_of!((*POOL.0.get())[self.slot].code) as u64 }
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
    address | 1 | USER | (1 << 4) | permissions
}
fn physical<T>(value: &T) -> Result<u64, DomainRefusal> {
    boot::executable_physical_address(value as *const T as u64)
        .filter(|address| *address != 0 && *address & !ADDRESS == 0)
        .ok_or(DomainRefusal::InvalidMemory)
}
fn physical_mapping(address: u64) -> Result<u64, DomainRefusal> {
    let hhdm = HHDM.load(Ordering::Acquire);
    if hhdm == 0 || address == 0 || address & !ADDRESS != 0 {
        return Err(DomainRefusal::InvalidMemory);
    }
    hhdm.checked_add(address)
        .ok_or(DomainRefusal::InvalidMemory)
}
fn validate_root(entry: u64, level: u32, remaining: &mut u32) -> Result<(), DomainRefusal> {
    *remaining = remaining.checked_sub(1).ok_or(DomainRefusal::Unsupported)?;
    if entry == 0 {
        return Ok(());
    }
    if entry & 1 == 0 {
        if level == 0 || entry & !ADDRESS != 0 {
            return Err(DomainRefusal::Unsupported);
        }
        let table = physical_mapping(entry)? as *const u64;
        for index in 0..512 {
            validate_root(
                unsafe { table.add(index).read_volatile() },
                level - 1,
                remaining,
            )?;
        }
    } else {
        let allowed = ADDRESS | 0x17f | (7 << 61);
        if entry & USER != 0 || entry & !allowed != 0 || level == 3 {
            return Err(DomainRefusal::Unsupported);
        }
        let address = if level == 0 {
            entry & ADDRESS
        } else {
            if entry & (1 << 6) == 0 || entry & (1 << 12) == 0 {
                return Err(DomainRefusal::Unsupported);
            }
            (entry & ADDRESS) & !(1 << 12)
        };
        if address & ((1_u64 << (12 + 9 * level)) - 1) != 0 {
            return Err(DomainRefusal::Unsupported);
        }
    }
    Ok(())
}
pub(super) fn enable_no_execute() -> Result<(), DomainRefusal> {
    let cpu: usize;
    unsafe {
        core::arch::asm!("cpucfg {0}, {1}",out(reg)cpu,in(reg)1usize,options(nostack));
    }
    if cpu & (1 << 22) == 0
        || (cpu >> 12) & 0xff != 47
        || (cpu >> 4) & 0xff > 47
        || super::read_csr::<0>() & 0x1fb != 0xb0
        || super::read_csr::<2>() != 1
        || super::read_csr::<0x1c>() != PWCL
        || super::read_csr::<0x1d>() != PWCH
        || super::read_csr::<0x1e>() != 12
        || super::read_csr::<0x88>() == 0
        || super::read_csr::<0x21>() & 15 < 5
        || super::read_csr::<0x180>() != 0x11
        || super::read_csr::<0x181>() != 0
        || super::read_csr::<0x182>() != 0
        || super::read_csr::<0x183>() != 0
    {
        Err(DomainRefusal::Unsupported)
    } else {
        Ok(())
    }
}
