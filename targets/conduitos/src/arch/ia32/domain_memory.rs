//! Fixed PAE address spaces: Root identity mappings and a disjoint user range.
use crate::domain_layout::{RETAINED_BYTES, RETAINED_PAGE, STACK_BYTES, STACK_PAGE};
use crate::{
    domain_image::{DomainImage, IA32_USER_TEXT_START, MAXIMUM_IMAGE_BYTES},
    protected_region::DomainRefusal,
};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU8, Ordering},
};

use super::ordinary_domain::TextFrame;
pub(super) const USER_FRAME: u32 = (IA32_USER_TEXT_START + MAXIMUM_IMAGE_BYTES) as u32;
pub(super) const USER_STACK_TOP: u32 = IA32_USER_TEXT_START as u32 + 0x48000 - 20;
const PAGE: usize = 4096;
const NX: u64 = 1 << 63;

#[repr(C, align(4096))]
struct Table([u64; 512]);
#[repr(C, align(4096))]
struct Bytes<const N: usize>([u8; N]);
#[repr(C, align(16))]
struct FloatingState([u8; 512]);
#[repr(C)]
struct Slot {
    pdpt: Table,
    directories: [Table; 4],
    user: Table,
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
            pdpt: Table([0; 512]),
            directories: [const { Table([0; 512]) }; 4],
            user: Table([0; 512]),
            code: Bytes([0; 131072]),
            frame: Bytes([0; 4096]),
            stack: Bytes([0; STACK_BYTES]),
            retained: Bytes([0; RETAINED_BYTES]),
            trap: Bytes([0; 16384]),
            root_floating: FloatingState([0; 512]),
            user_floating: FloatingState([0; 512]),
        }
    }
}
struct Pool(UnsafeCell<[Slot; 2]>);
unsafe impl Sync for Pool {}
static POOL: Pool = Pool(UnsafeCell::new([Slot::empty(), Slot::empty()]));
static OWNED: AtomicU8 = AtomicU8::new(0);

pub(super) struct AddressSpace {
    slot: usize,
    pub cr3: u32,
    pub entry: u32,
    cleared: bool,
}
impl AddressSpace {
    pub const RESERVED_BYTES: u32 = core::mem::size_of::<Slot>() as u32;

    pub fn install(image: &DomainImage<'_>) -> Result<Self, DomainRefusal> {
        let cr0: u32;
        unsafe {
            core::arch::asm!("mov {0:e}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        }
        // This backend is for the protected-mode, identity-mapped PC entrance.
        // Do not reinterpret an inherited paging regime or silently replace it.
        if cr0 & (1 << 31) != 0 {
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
            cr3: 0,
            entry: 0,
            cleared: false,
        };
        let memory = unsafe { &mut (*POOL.0.get())[slot] };
        if core::ptr::addr_of!(*memory) as usize + core::mem::size_of::<Slot>()
            > IA32_USER_TEXT_START as usize
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        for directory in 0..4 {
            // PAE PDPTE bits 1 and 2 are reserved; permissions live below it.
            memory.pdpt.0[directory] = address(&memory.directories[directory]) | 1;
            for page in 0..512 {
                memory.directories[directory].0[page] =
                    (((directory * 512 + page) as u64) << 21) | 0x83;
            }
        }
        memory.directories[1].0[0] = address(&memory.user) | 7;
        for segment in &image.segments {
            let offset = segment
                .address
                .checked_sub(IA32_USER_TEXT_START)
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or(DomainRefusal::InvalidMemory)?;
            let end = offset
                .checked_add(segment.bytes.len())
                .ok_or(DomainRefusal::InvalidMemory)?;
            let destination = memory
                .code
                .0
                .get_mut(offset..end)
                .ok_or(DomainRefusal::InvalidMemory)?;
            destination.copy_from_slice(segment.bytes);
            for page in (offset..offset + segment.mapped_bytes() as usize).step_by(PAGE) {
                memory.user.0[page / PAGE] = address(&memory.code) + page as u64
                    | 5
                    | if segment.executable { 0 } else { NX };
            }
        }
        memory.user.0[32] = address(&memory.frame) | 7 | NX;
        // The independently compiled image uses the i386 C calling convention.
        memory.stack.0[STACK_BYTES - 16..STACK_BYTES - 12]
            .copy_from_slice(&USER_FRAME.to_le_bytes());
        memory.user_floating.0[..2].copy_from_slice(&0x037fu16.to_le_bytes());
        memory.user_floating.0[24..28].copy_from_slice(&0x1f80u32.to_le_bytes());
        for page in 0..STACK_BYTES / PAGE {
            memory.user.0[STACK_PAGE + page] =
                address(&memory.stack) + (page * PAGE) as u64 | 7 | NX;
        }
        for page in 0..RETAINED_BYTES / PAGE {
            memory.user.0[RETAINED_PAGE + page] =
                address(&memory.retained) + (page * PAGE) as u64 | 7 | NX;
        }
        space.cr3 = address(&memory.pdpt) as u32;
        space.entry = u32::try_from(image.entry).map_err(|_| DomainRefusal::InvalidMemory)?;
        Ok(space)
    }

    pub fn frame(&mut self) -> &mut TextFrame {
        unsafe {
            &mut *((*POOL.0.get())[self.slot]
                .frame
                .0
                .as_mut_ptr()
                .cast::<TextFrame>())
        }
    }
    pub fn trap_stack_top(&self) -> u32 {
        unsafe { address(&(*POOL.0.get())[self.slot].trap) as u32 + 16384 }
    }
    pub fn floating_states(&self) -> (u32, u32) {
        unsafe {
            let slot = &(*POOL.0.get())[self.slot];
            (
                address(&slot.root_floating) as u32,
                address(&slot.user_floating) as u32,
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
fn address<T>(value: &T) -> u64 {
    value as *const T as usize as u64
}

pub(super) fn enable_no_execute() -> Result<(), DomainRefusal> {
    let features = core::arch::x86::__cpuid(1);
    let maximum = core::arch::x86::__cpuid(0x80000000).eax;
    if features.edx & ((1 << 6) | (1 << 24) | (1 << 26)) != ((1 << 6) | (1 << 24) | (1 << 26))
        || maximum < 0x80000001
        || core::arch::x86::__cpuid(0x80000001).edx & (1 << 20) == 0
    {
        return Err(DomainRefusal::Unsupported);
    }
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") 0xc0000080u32, out("eax") low, out("edx") high, options(nostack));
        core::arch::asm!("wrmsr", in("ecx") 0xc0000080u32, in("eax") low | (1 << 11), in("edx") high, options(nostack));
    }
    Ok(())
}
