//! Fixed-capacity 2 MiB device mappings shared by x86 platform providers.

use core::{
    arch::asm,
    ptr::write_volatile,
    sync::atomic::{AtomicU64, Ordering},
};

const PAGE_BYTES: u64 = 0x20_0000;
const SLOT_COUNT: usize = 4;
const PML4_INDEX: usize = 509;
const VIRTUAL_BASE: u64 = 0xffff_fe80_0000_0000;
const PML4_SPAN: u64 = 0x80_0000_0000;

#[repr(C, align(4096))]
struct PageTable([u64; 512]);

static mut PDPTS: [PageTable; SLOT_COUNT] = [const { PageTable([0; 512]) }; SLOT_COUNT];
static mut PDS: [PageTable; SLOT_COUNT] = [const { PageTable([0; 512]) }; SLOT_COUNT];
const EMPTY_PAGE: u64 = u64::MAX;
static PHYSICAL_PAGES: [AtomicU64; SLOT_COUNT] = [const { AtomicU64::new(EMPTY_PAGE) }; SLOT_COUNT];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioRefusal {
    Address,
    PageTables,
    Capacity,
}

pub(super) fn map(
    physical: u64,
    hhdm: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<usize, MmioRefusal> {
    let page_offset = physical & (PAGE_BYTES - 1);
    let physical_page = physical & !(PAGE_BYTES - 1);
    let mut slot = None;
    for (index, page) in PHYSICAL_PAGES.iter().enumerate() {
        let current = page.load(Ordering::Acquire);
        if current == physical_page
            || current == EMPTY_PAGE
                && page
                    .compare_exchange(
                        EMPTY_PAGE,
                        physical_page,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
        {
            slot = Some(index);
            break;
        }
    }
    let slot = slot.ok_or(MmioRefusal::Capacity)?;
    install_slot(slot, physical_page, hhdm, image_virtual_to_physical)?;
    let virtual_address = VIRTUAL_BASE
        .checked_sub((slot as u64) * PML4_SPAN)
        .and_then(|base| base.checked_add(page_offset))
        .ok_or(MmioRefusal::Address)?;
    usize::try_from(virtual_address).map_err(|_| MmioRefusal::Address)
}

fn install_slot(
    slot: usize,
    physical_page: u64,
    hhdm: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<(), MmioRefusal> {
    unsafe {
        let pdpt_physical = image_virtual_to_physical(core::ptr::addr_of!(PDPTS[slot]) as u64)
            .ok_or(MmioRefusal::PageTables)?;
        let pd_physical = image_virtual_to_physical(core::ptr::addr_of!(PDS[slot]) as u64)
            .ok_or(MmioRefusal::PageTables)?;
        if pdpt_physical & 0xfff != 0 || pd_physical & 0xfff != 0 {
            return Err(MmioRefusal::PageTables);
        }
        let cr3 = current_cr3();
        let pml4_virtual = hhdm.checked_add(cr3 & !0xfff).ok_or(MmioRefusal::Address)?;
        let pml4 = usize::try_from(pml4_virtual).map_err(|_| MmioRefusal::Address)? as *mut u64;
        PDPTS[slot].0 = [0; 512];
        PDS[slot].0 = [0; 512];
        write_volatile(core::ptr::addr_of_mut!(PDPTS[slot].0[0]), pd_physical | 0x3);
        write_volatile(
            core::ptr::addr_of_mut!(PDS[slot].0[0]),
            physical_page | 0x9b,
        );
        write_volatile(pml4.add(PML4_INDEX - slot), pdpt_physical | 0x3);
        flush();
        Ok(())
    }
}

fn current_cr3() -> u64 {
    let cr3: u64;
    unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nostack, nomem, preserves_flags)) };
    cr3
}

unsafe fn flush() {
    let cr3 = current_cr3();
    unsafe { asm!("mov cr3, {}", in(reg) cr3, options(nostack, preserves_flags)) };
}
