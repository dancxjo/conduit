//! One independently owned compositor-service address space, separate from the
//! ordinary text/timer pool. Every private byte and page-table page is charged.
use super::domain_transition::EntryContext;
use crate::{
    boot,
    protected_region::DomainRefusal,
    service_memory::{MemoryRange, ServiceImage, ServiceMemoryLayout},
};
use core::{
    arch::asm,
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

const LAYOUT: ServiceMemoryLayout = ServiceMemoryLayout::compositor();
const PAGE: usize = 4096;
const TABLES: usize = (LAYOUT.scratch().end() as usize).div_ceil(2 * 1024 * 1024);
const NX: u64 = 1 << 63;
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct Table([u64; 512]);
#[repr(C, align(4096))]
struct Bytes<const N: usize>([u8; N]);
#[repr(C)]
struct Slot {
    pml4: Table,
    pdpt: Table,
    pd: Table,
    pt: [Table; TABLES],
    image: Bytes<{ LAYOUT.image().bytes() as usize }>,
    exchange: Bytes<{ LAYOUT.exchange().bytes() as usize }>,
    output: Bytes<{ LAYOUT.output().bytes() as usize }>,
    stack: Bytes<{ LAYOUT.stack().bytes() as usize }>,
    metadata: Bytes<{ LAYOUT.metadata().bytes() as usize }>,
    pixels: Bytes<{ LAYOUT.pixels().bytes() as usize }>,
    scratch: Bytes<{ LAYOUT.scratch().bytes() as usize }>,
    trap: Bytes<16384>,
}
impl Slot {
    const fn empty() -> Self {
        Self {
            pml4: Table([0; 512]),
            pdpt: Table([0; 512]),
            pd: Table([0; 512]),
            pt: [Table([0; 512]); TABLES],
            image: Bytes([0; LAYOUT.image().bytes() as usize]),
            exchange: Bytes([0; LAYOUT.exchange().bytes() as usize]),
            output: Bytes([0; LAYOUT.output().bytes() as usize]),
            stack: Bytes([0; LAYOUT.stack().bytes() as usize]),
            metadata: Bytes([0; LAYOUT.metadata().bytes() as usize]),
            pixels: Bytes([0; LAYOUT.pixels().bytes() as usize]),
            scratch: Bytes([0; LAYOUT.scratch().bytes() as usize]),
            trap: Bytes([0; 16384]),
        }
    }
    fn clear(&mut self) {
        self.pml4.0.fill(0);
        self.pdpt.0.fill(0);
        self.pd.0.fill(0);
        for table in &mut self.pt {
            table.0.fill(0);
        }
        self.image.0.fill(0);
        self.exchange.0.fill(0);
        self.output.0.fill(0);
        self.stack.0.fill(0);
        self.metadata.0.fill(0);
        self.pixels.0.fill(0);
        self.scratch.0.fill(0);
        self.trap.0.fill(0);
    }
}
struct Pool(UnsafeCell<Slot>);
// OWNED admits exactly one AddressSpace; it alone accesses the slot until Drop.
unsafe impl Sync for Pool {}
static POOL: Pool = Pool(UnsafeCell::new(Slot::empty()));
static OWNED: AtomicBool = AtomicBool::new(false);

pub(super) struct AddressSpace {
    cr3: u64,
    entry: u64,
    cleared: bool,
}
impl AddressSpace {
    pub const RESERVED_BYTES: u32 = core::mem::size_of::<Slot>() as u32;
    pub fn install(image: &ServiceImage<'_>) -> Result<Self, DomainRefusal> {
        OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| DomainRefusal::InvalidMemory)?;
        // SAFETY: this successful claim exclusively owns the static slot.
        let slot = unsafe { &mut *POOL.0.get() };
        let result = install(slot, image);
        if result.is_err() {
            slot.clear();
            OWNED.store(false, Ordering::Release);
        }
        result
    }
    pub fn exchange(&mut self) -> Result<&mut [u8], DomainRefusal> {
        if self.cleared {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        // SAFETY: the unique AddressSpace owns this slot and mutable window.
        Ok(unsafe { &mut (*POOL.0.get()).exchange.0 })
    }
    pub fn output(&self) -> Result<&[u8], DomainRefusal> {
        if self.cleared {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        // SAFETY: no domain runs concurrently with its Root caller.
        Ok(unsafe { &(*POOL.0.get()).output.0 })
    }
    pub fn context(&self) -> Result<EntryContext, DomainRefusal> {
        if self.cleared {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        Ok(EntryContext {
            entry: self.entry,
            cr3: self.cr3,
            frame: LAYOUT.exchange().start(),
            stack: LAYOUT.stack().end() - 8,
            // SAFETY: this slot's trap stack is exclusively owned and remains
            // supervisor-only in every user mapping.
            trap_stack: unsafe { core::ptr::addr_of!((*POOL.0.get()).trap.0) as u64 + 16384 },
        })
    }
    pub fn quarantine(&mut self) -> u32 {
        if self.cleared {
            return 0;
        }
        // SAFETY: entry returned to Root before quarantine; ownership is held
        // through clearing. No sibling can acquire this slot until Drop.
        unsafe {
            (*POOL.0.get()).clear();
        }
        self.cleared = true;
        Self::RESERVED_BYTES
    }
}
impl Drop for AddressSpace {
    fn drop(&mut self) {
        self.quarantine();
        OWNED.store(false, Ordering::Release);
    }
}

fn install(slot: &mut Slot, image: &ServiceImage<'_>) -> Result<AddressSpace, DomainRefusal> {
    if image.layout() != LAYOUT {
        return Err(DomainRefusal::InvalidMemory);
    }
    slot.clear();
    let hhdm = super::domain_memory::hhdm_offset()?;
    let current_cr3: u64;
    // SAFETY: installation runs only in Root.
    unsafe {
        asm!("mov {}, cr3", out(reg) current_cr3, options(nomem, nostack, preserves_flags));
    }
    let root = hhdm
        .checked_add(current_cr3 & !0xfff)
        .ok_or(DomainRefusal::InvalidMemory)? as *const u64;
    for index in 0..512 {
        // SAFETY: the current CR3 is a live Root page table mapped by the HHDM.
        slot.pml4.0[index] = unsafe { root.add(index).read() } & !4;
    }
    slot.pml4.0[0] = physical(&slot.pdpt)? | 7;
    slot.pdpt.0[0] = physical(&slot.pd)? | 7;
    let image_physical = physical(&slot.image)?;
    for segment in &image.image().segments {
        let offset = usize::try_from(
            segment
                .address
                .checked_sub(LAYOUT.image().start())
                .ok_or(DomainRefusal::InvalidMemory)?,
        )
        .map_err(|_| DomainRefusal::InvalidMemory)?;
        let end = offset
            .checked_add(segment.bytes.len())
            .ok_or(DomainRefusal::InvalidMemory)?;
        slot.image
            .0
            .get_mut(offset..end)
            .ok_or(DomainRefusal::InvalidMemory)?
            .copy_from_slice(segment.bytes);
        map(
            slot,
            segment.address,
            segment.mapped_bytes(),
            image_physical + offset as u64,
            5 | if segment.executable { 0 } else { NX },
        )?;
    }
    let private = [
        (LAYOUT.exchange(), physical(&slot.exchange)?),
        (LAYOUT.output(), physical(&slot.output)?),
        (LAYOUT.stack(), physical(&slot.stack)?),
        (LAYOUT.metadata(), physical(&slot.metadata)?),
        (LAYOUT.pixels(), physical(&slot.pixels)?),
        (LAYOUT.scratch(), physical(&slot.scratch)?),
    ];
    for (range, base) in private {
        map_private(slot, range, base)?;
    }
    Ok(AddressSpace {
        cr3: physical(&slot.pml4)?,
        entry: image.image().entry,
        cleared: false,
    })
}
fn map_private(slot: &mut Slot, range: MemoryRange, base: u64) -> Result<(), DomainRefusal> {
    map(slot, range.start(), range.bytes(), base, 7 | NX)
}
fn map(
    slot: &mut Slot,
    start: u64,
    bytes: u64,
    base: u64,
    flags: u64,
) -> Result<(), DomainRefusal> {
    if start % PAGE as u64 != 0 || base % PAGE as u64 != 0 || bytes % PAGE as u64 != 0 {
        return Err(DomainRefusal::InvalidMemory);
    }
    for offset in (0..bytes).step_by(PAGE) {
        let virtual_address = start
            .checked_add(offset)
            .ok_or(DomainRefusal::InvalidMemory)?;
        let directory =
            usize::try_from(virtual_address >> 21).map_err(|_| DomainRefusal::InvalidMemory)?;
        let index = (virtual_address as usize >> 12) & 511;
        if directory >= TABLES || slot.pt[directory].0[index] != 0 {
            return Err(DomainRefusal::InvalidMemory);
        }
        let table = physical(&slot.pt[directory])?;
        slot.pd.0[directory] = table | 7;
        slot.pt[directory].0[index] = base
            .checked_add(offset)
            .ok_or(DomainRefusal::InvalidMemory)?
            | flags;
    }
    Ok(())
}
fn physical<T>(value: &T) -> Result<u64, DomainRefusal> {
    boot::executable_physical_address(value as *const T as u64)
        .filter(|address| address & 0xfff == 0)
        .ok_or(DomainRefusal::InvalidMemory)
}
