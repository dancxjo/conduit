//! Fixed, disjoint address spaces for two admitted ordinary regions.
use crate::{
    boot,
    domain_image::{DomainImage, MAXIMUM_IMAGE_BYTES, USER_TEXT_START},
    protected_region::DomainRefusal,
};
use core::{
    arch::asm,
    cell::UnsafeCell,
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
};

pub(super) const USER_FRAME: u64 = USER_TEXT_START + MAXIMUM_IMAGE_BYTES;
pub(super) const USER_STACK_TOP: u64 = USER_TEXT_START + 0x24_000;
const NX: u64 = 1 << 63;
const PAGE: usize = 4096;

#[repr(C, align(4096))]
struct Table([u64; 512]);
#[repr(C, align(4096))]
struct Bytes<const N: usize>([u8; N]);

#[repr(C)]
struct Slot {
    pml4: Table,
    pdpt: Table,
    pd: Table,
    pt: Table,
    code: Bytes<65536>,
    frame: Bytes<4096>,
    stack: Bytes<16384>,
    trap: Bytes<16384>,
}
impl Slot {
    const fn empty() -> Self {
        Self {
            pml4: Table([0; 512]),
            pdpt: Table([0; 512]),
            pd: Table([0; 512]),
            pt: Table([0; 512]),
            code: Bytes([0; 65536]),
            frame: Bytes([0; 4096]),
            stack: Bytes([0; 16384]),
            trap: Bytes([0; 16384]),
        }
    }
}
struct Pool(UnsafeCell<[Slot; 2]>);
unsafe impl Sync for Pool {}
static POOL: Pool = Pool(UnsafeCell::new([Slot::empty(), Slot::empty()]));
static OWNED: AtomicU8 = AtomicU8::new(0);
static HHDM: AtomicU64 = AtomicU64::new(0);

pub(super) fn initialize(hhdm: u64) {
    HHDM.store(hhdm, Ordering::Release);
}

pub(super) struct AddressSpace {
    pub slot: usize,
    pub cr3: u64,
    pub entry: u64,
    cleared: bool,
}

impl AddressSpace {
    pub fn install(image: &DomainImage<'_>) -> Result<Self, DomainRefusal> {
        let hhdm = HHDM.load(Ordering::Acquire);
        if hhdm == 0 {
            return Err(DomainRefusal::Unsupported);
        }
        let slot = (0..2)
            .find(|index| {
                let bit = 1 << index;
                OWNED.fetch_or(bit, Ordering::AcqRel) & bit == 0
            })
            .ok_or(DomainRefusal::InvalidMemory)?;
        let result = unsafe { install(slot, image, hhdm) };
        if result.is_err() {
            OWNED.fetch_and(!(1 << slot), Ordering::Release);
        }
        result
    }

    pub fn frame(&mut self) -> &mut super::ordinary_domain::TextFrame {
        unsafe {
            &mut *((*POOL.0.get())[self.slot]
                .frame
                .0
                .as_mut_ptr()
                .cast::<super::ordinary_domain::TextFrame>())
        }
    }
    pub fn trap_stack_top(&self) -> u64 {
        unsafe { core::ptr::addr_of!((*POOL.0.get())[self.slot].trap.0) as u64 + 16384 }
    }
    pub fn quarantine(&mut self) -> u32 {
        if self.cleared {
            return 0;
        }
        unsafe {
            let slot = &mut (*POOL.0.get())[self.slot];
            slot.frame.0.fill(0);
            slot.stack.0.fill(0);
            slot.trap.0.fill(0);
            slot.code.0.fill(0);
            slot.pml4.0.fill(0);
            slot.pdpt.0.fill(0);
            slot.pd.0.fill(0);
            slot.pt.0.fill(0);
        }
        self.cleared = true;
        core::mem::size_of::<Slot>() as u32
    }
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        self.quarantine();
        OWNED.fetch_and(!(1 << self.slot), Ordering::Release);
    }
}

unsafe fn install(
    slot: usize,
    image: &DomainImage<'_>,
    hhdm: u64,
) -> Result<AddressSpace, DomainRefusal> {
    let slot_memory = unsafe { &mut (*POOL.0.get())[slot] };
    let current_cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) current_cr3, options(nomem, nostack, preserves_flags));
    }
    let current = (hhdm + (current_cr3 & !0xfff)) as *const u64;
    for index in 0..512 {
        // All inherited Root mappings remain supervisor-only at the top level.
        slot_memory.pml4.0[index] = unsafe { current.add(index).read() } & !4;
    }
    slot_memory.pdpt.0.fill(0);
    slot_memory.pd.0.fill(0);
    slot_memory.pt.0.fill(0);
    slot_memory.code.0.fill(0);
    slot_memory.frame.0.fill(0);
    slot_memory.stack.0.fill(0);
    slot_memory.pml4.0[0] = physical(&slot_memory.pdpt)? | 7;
    slot_memory.pdpt.0[0] = physical(&slot_memory.pd)? | 7;
    slot_memory.pd.0[2] = physical(&slot_memory.pt)? | 7;
    for segment in &image.segments {
        let offset = usize::try_from(
            segment
                .address
                .checked_sub(USER_TEXT_START)
                .ok_or(DomainRefusal::InvalidMemory)?,
        )
        .map_err(|_| DomainRefusal::InvalidMemory)?;
        let end = offset
            .checked_add(segment.bytes.len())
            .filter(|end| *end <= 65536)
            .ok_or(DomainRefusal::InvalidMemory)?;
        slot_memory.code.0[offset..end].copy_from_slice(segment.bytes);
        for page in 0..segment.mapped_bytes() as usize / PAGE {
            let index = offset / PAGE + page;
            let address = physical(&slot_memory.code)? + (index * PAGE) as u64;
            slot_memory.pt.0[index] = address | 5 | if segment.executable { 0 } else { NX };
        }
    }
    slot_memory.pt.0[16] = physical(&slot_memory.frame)? | 7 | NX;
    for page in 0..4 {
        slot_memory.pt.0[32 + page] = physical(&slot_memory.stack)? + (page * PAGE) as u64 | 7 | NX;
    }
    Ok(AddressSpace {
        slot,
        cr3: physical(&slot_memory.pml4)?,
        entry: image.entry,
        cleared: false,
    })
}

fn physical<T>(value: &T) -> Result<u64, DomainRefusal> {
    boot::executable_physical_address(value as *const T as u64)
        .filter(|address| address & 0xfff == 0)
        .ok_or(DomainRefusal::InvalidMemory)
}

pub(super) fn enable_no_execute() -> Result<(), DomainRefusal> {
    if core::arch::x86_64::__cpuid(0x8000_0001).edx & (1 << 20) == 0 {
        return Err(DomainRefusal::Unsupported);
    }
    let low: u32;
    let high: u32;
    unsafe {
        asm!("rdmsr", in("ecx") 0xc000_0080u32, out("eax") low, out("edx") high,
            options(nostack, nomem));
        asm!("wrmsr", in("ecx") 0xc000_0080u32, in("eax") (low | (1 << 11)) & !1, in("edx") high,
            options(nostack, nomem));
        // Root exposes only the checked interrupt gate; disable alternate entries.
        asm!("wrmsr", in("ecx") 0x174u32, in("eax") 0u32, in("edx") 0u32,
            options(nostack, nomem));
    }
    Ok(())
}
