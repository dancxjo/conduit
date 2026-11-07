//! Fixed identity translation for BCM2835 RAM and peripheral accesses.
//!
//! With translation disabled, data accesses are Strongly-ordered. ARM1176
//! exclusive loads/stores must instead address Normal memory. This map supplies
//! those attributes and write-through instruction/data caching, without claiming
//! Source isolation. GPU-reserved RAM remains noncacheable.

use core::{
    cell::UnsafeCell,
    sync::atomic::{Ordering, compiler_fence},
};

const SECTIONS: usize = 4096;
#[path = "memory_attributes.rs"]
mod attributes;

#[repr(C, align(16384))]
struct TranslationTable(UnsafeCell<[u32; SECTIONS]>);

// Only early boot writes the table, before enabling interrupts or translation.
unsafe impl Sync for TranslationTable {}

static TABLE: TranslationTable = TranslationTable(UnsafeCell::new([0; SECTIONS]));

pub(super) fn initialize() {
    let table = TABLE.0.get().cast::<u32>();
    for index in 0..SECTIONS {
        let address = (index as u32) << 20;
        // SAFETY: each entry belongs to our aligned, boot-owned static table.
        unsafe {
            table
                .add(index)
                .write_volatile(attributes::section_descriptor(address));
        }
    }
    compiler_fence(Ordering::SeqCst);
    let control: u32;
    // SAFETY: Root is in privileged SVC mode, IRQs are disabled, and the
    // firmware enters with translation and caches disabled. Code, stacks,
    // vectors, table, and MMIO retain their exact physical addresses.
    unsafe {
        core::arch::asm!(
            "mcr p15, 0, {zero}, c7, c10, 4", // Drain writes before table walks.
            "mcr p15, 0, {zero}, c2, c0, 2", // TTBCR.N=0: TTBR0 covers all VA.
            "mcr p15, 0, {table}, c2, c0, 0",
            "mcr p15, 0, {domain}, c3, c0, 0", // Domain 0 client; check AP.
            "mcr p15, 0, {zero}, c8, c7, 0", // Invalidate unified TLB.
            "mcr p15, 0, {zero}, c7, c5, 0", // Invalidate instruction cache.
            "mcr p15, 0, {zero}, c7, c6, 0", // Invalidate data cache before enabling it.
            "mrc p15, 0, {control}, c1, c0, 0",
            zero = in(reg) 0_u32,
            table = in(reg) table as u32,
            domain = in(reg) 1_u32,
            control = out(reg) control,
            options(nostack),
        );
        let control = attributes::control(control);
        core::arch::asm!(
            "mcr p15, 0, {control}, c1, c0, 0",
            "mcr p15, 0, {zero}, c7, c5, 4", // Flush the prefetch pipeline.
            control = in(reg) control,
            zero = in(reg) 0_u32,
            options(nostack),
        );
    }
    compiler_fence(Ordering::SeqCst);
}
