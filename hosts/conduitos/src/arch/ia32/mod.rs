//! QEMU IA-32 A2 mechanisms below the generic machine/Base seam.

use core::sync::atomic::{AtomicBool, Ordering};

pub const PIT_IRQ: u8 = 32;
static FACT_PRESENT: AtomicBool = AtomicBool::new(false);
static FACT_OVERFLOW: AtomicBool = AtomicBool::new(false);
static mut IDT: [u64; 256] = [0; 256];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptFact {
    Timer,
    WrongSource(u8),
    Overflow,
}

core::arch::global_asm!(
    r#"
.section .text.conduitos_ia32_irq,"ax",@progbits
.global conduitos_ia32_irq_entry
conduitos_ia32_irq_entry:
    pushad
    call conduitos_ia32_irq_handler
    popad
    iretd
"#
);

pub fn initialize_machine() {
    disable_interrupts();
    let handler = conduitos_ia32_irq_entry as *const () as usize as u32;
    let selector: u16;
    unsafe {
        core::arch::asm!("mov {0:x}, cs", out(reg) selector, options(nomem, nostack, preserves_flags))
    };
    let gate = u64::from(handler & 0xffff)
        | (u64::from(selector) << 16)
        | (0x8e_u64 << 40)
        | (u64::from(handler >> 16) << 48);
    unsafe {
        IDT[PIT_IRQ as usize] = gate;
        let descriptor = DescriptorTable {
            limit: (core::mem::size_of::<[u64; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u32,
        };
        core::arch::asm!("lidt [{0}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags));
        remap_pic();
    }
}

pub fn timer_arm() {
    const TICKS: u16 = 1193;
    unsafe {
        outb(0x43, 0x30);
        outb(0x40, TICKS as u8);
        outb(0x40, (TICKS >> 8) as u8);
    }
}

pub fn enable_interrupts() {
    unsafe { core::arch::asm!("sti", options(nomem, nostack, preserves_flags)) }
}

pub fn disable_interrupts() {
    unsafe { core::arch::asm!("cli", options(nomem, nostack, preserves_flags)) }
}

pub fn interruptible_idle() {
    unsafe { core::arch::asm!("hlt", options(nomem, nostack)) }
}

pub fn pop_interrupt() -> Option<InterruptFact> {
    if FACT_OVERFLOW.swap(false, Ordering::AcqRel) {
        return Some(InterruptFact::Overflow);
    }
    FACT_PRESENT
        .swap(false, Ordering::AcqRel)
        .then_some(InterruptFact::Timer)
}

pub fn present(bytes: &[u8]) {
    for byte in bytes {
        unsafe { outb(0xe9, *byte) };
    }
}

pub fn read_counter() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)) };
    u64::from(low) | (u64::from(high) << 32)
}

unsafe extern "C" {
    fn conduitos_ia32_irq_entry();
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_irq_handler() {
    if FACT_PRESENT.swap(true, Ordering::AcqRel) {
        FACT_OVERFLOW.store(true, Ordering::Release);
    }
    unsafe { outb(0x20, 0x20) };
}

#[repr(C, packed)]
struct DescriptorTable {
    limit: u16,
    base: u32,
}

unsafe fn remap_pic() {
    unsafe {
        outb(0x20, 0x11);
        outb(0xa0, 0x11);
        outb(0x21, 0x20);
        outb(0xa1, 0x28);
        outb(0x21, 0x04);
        outb(0xa1, 0x02);
        outb(0x21, 0x01);
        outb(0xa1, 0x01);
        outb(0x21, 0xfe);
        outb(0xa1, 0xff);
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags))
    };
}
