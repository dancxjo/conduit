//! BCM2835 machine mechanisms for the ARMv6 Raspberry Pi target.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering, compiler_fence},
};

mod providers;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer};

const GPIO_BASE: usize = 0x2020_0000;
const UART0_BASE: usize = 0x2020_1000;
const SYSTEM_TIMER_BASE: usize = 0x2000_3000;
const INTERRUPT_BASE: usize = 0x2000_b000;
const MAILBOX_BASE: usize = 0x2000_b880;
const MAILBOX_PROPERTY_CHANNEL: u32 = 8;
const MAILBOX_EMPTY: u32 = 1 << 30;
const MAILBOX_FULL: u32 = 1 << 31;
const MAILBOX_POLL_LIMIT: usize = 1_000_000;
const SYSTEM_TIMER_COMPARE_1: u32 = 1 << 1;
const FACT_CAPACITY: usize = 1;

static FACT_PRESENT: AtomicBool = AtomicBool::new(false);
static FACT_OVERFLOW: AtomicBool = AtomicBool::new(false);

#[repr(C, align(16))]
struct MailboxMessage(UnsafeCell<[u32; 7]>);

unsafe impl Sync for MailboxMessage {}

static BOARD_REVISION_MESSAGE: MailboxMessage = MailboxMessage(UnsafeCell::new([0; 7]));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptFact {
    Timer,
    WrongSource,
    Overflow,
}

pub fn initialize_machine() {
    disable_interrupts();
    initialize_pl011();
    install_vectors();
    unsafe {
        write32(INTERRUPT_BASE + 0x21c, u32::MAX);
        write32(INTERRUPT_BASE + 0x220, u32::MAX);
        write32(SYSTEM_TIMER_BASE, SYSTEM_TIMER_COMPARE_1);
        write32(INTERRUPT_BASE + 0x210, SYSTEM_TIMER_COMPARE_1);
    }
}

pub fn timer_arm() {
    let deadline = unsafe { read32(SYSTEM_TIMER_BASE + 0x04) }.wrapping_add(1_000);
    unsafe {
        write32(SYSTEM_TIMER_BASE, SYSTEM_TIMER_COMPARE_1);
        write32(SYSTEM_TIMER_BASE + 0x10, deadline);
    }
}

pub fn enable_interrupts() {
    unsafe { core::arch::asm!("cpsie i", options(nomem, nostack, preserves_flags)) }
}

pub fn disable_interrupts() {
    unsafe { core::arch::asm!("cpsid i", options(nomem, nostack, preserves_flags)) }
}

pub fn interrupts_enabled() -> bool {
    let cpsr: u32;
    unsafe { core::arch::asm!("mrs {cpsr}, cpsr", cpsr = out(reg) cpsr, options(nomem, nostack)) };
    cpsr & (1 << 7) == 0
}

pub fn interruptible_idle() {
    unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
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
    for &byte in bytes {
        while unsafe { read32(UART0_BASE + 0x18) } & (1 << 5) != 0 {
            core::hint::spin_loop();
        }
        unsafe { write32(UART0_BASE, u32::from(byte)) };
    }
}

pub fn read_counter() -> u64 {
    u64::from(unsafe { read32(SYSTEM_TIMER_BASE + 0x04) })
}

/// Returns the VideoCore firmware's board-revision observation when the
/// bounded property-mailbox exchange completes successfully.
pub fn firmware_board_revision() -> Option<u32> {
    let message = BOARD_REVISION_MESSAGE.0.get();
    unsafe {
        *message = [28, 0, 0x0001_0002, 4, 0, 0, 0];
    }
    compiler_fence(Ordering::SeqCst);
    let address = message as usize;
    if address >= 0x4000_0000 || address & 0xf != 0 {
        return None;
    }
    for _ in 0..MAILBOX_POLL_LIMIT {
        if unsafe { read32(MAILBOX_BASE + 0x18) } & MAILBOX_FULL == 0 {
            unsafe {
                write32(
                    MAILBOX_BASE + 0x20,
                    (address as u32 | 0x4000_0000) | MAILBOX_PROPERTY_CHANNEL,
                )
            };
            for _ in 0..MAILBOX_POLL_LIMIT {
                if unsafe { read32(MAILBOX_BASE + 0x18) } & MAILBOX_EMPTY != 0 {
                    continue;
                }
                let response = unsafe { read32(MAILBOX_BASE) };
                if response & 0xf != MAILBOX_PROPERTY_CHANNEL
                    || response & !0xf != (address as u32 | 0x4000_0000)
                {
                    continue;
                }
                compiler_fence(Ordering::SeqCst);
                let words = unsafe { &*message };
                return (words[1] == 0x8000_0000 && words[4] & 0x8000_0000 != 0)
                    .then_some(words[5]);
            }
            return None;
        }
    }
    None
}

fn initialize_pl011() {
    unsafe {
        write32(UART0_BASE + 0x30, 0);
        let mut function = read32(GPIO_BASE + 0x04);
        function &= !((7 << 12) | (7 << 15));
        function |= (4 << 12) | (4 << 15);
        write32(GPIO_BASE + 0x04, function);
        write32(GPIO_BASE + 0x94, 0);
        delay(150);
        write32(GPIO_BASE + 0x98, (1 << 14) | (1 << 15));
        delay(150);
        write32(GPIO_BASE + 0x98, 0);
        write32(UART0_BASE + 0x24, 1);
        write32(UART0_BASE + 0x28, 40);
        write32(UART0_BASE + 0x2c, (1 << 4) | (3 << 5));
        write32(UART0_BASE + 0x38, 0);
        write32(UART0_BASE + 0x44, 0x7ff);
        write32(UART0_BASE + 0x30, 1 | (1 << 8) | (1 << 9));
    }
}

fn install_vectors() {
    unsafe extern "C" {
        static __conduitos_armv6_vectors_start: u32;
        static __conduitos_armv6_vectors_end: u32;
    }
    let start = core::ptr::addr_of!(__conduitos_armv6_vectors_start);
    let end = core::ptr::addr_of!(__conduitos_armv6_vectors_end);
    let words = (end as usize - start as usize) / core::mem::size_of::<u32>();
    for index in 0..words {
        unsafe {
            core::ptr::write_volatile((index * 4) as *mut u32, core::ptr::read(start.add(index)));
        }
    }
    unsafe {
        core::arch::asm!(
            "mov r0, #0",
            "mcr p15, 0, r0, c7, c10, 4",
            "mcr p15, 0, r0, c7, c5, 4",
            out("r0") _,
            options(nostack)
        )
    };
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_armv6_irq_handler() {
    let pending = unsafe { read32(INTERRUPT_BASE + 0x204) };
    if pending & SYSTEM_TIMER_COMPARE_1 == 0 {
        FACT_OVERFLOW.store(true, Ordering::Release);
        return;
    }
    unsafe { write32(SYSTEM_TIMER_BASE, SYSTEM_TIMER_COMPARE_1) };
    if FACT_PRESENT.swap(true, Ordering::AcqRel) {
        FACT_OVERFLOW.store(true, Ordering::Release);
    }
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_armv6_unexpected_handler(
    cpsr: u32,
    exception_lr: u32,
    fault_status: u32,
    fault_address: u32,
) -> ! {
    present(b"CONDUIT_ARMV6_REFUSAL unexpected-exception cpsr=");
    present_hex(cpsr);
    present(b" lr=");
    present_hex(exception_lr);
    present(b" status=");
    present_hex(fault_status);
    present(b" address=");
    present_hex(fault_address);
    present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}

core::arch::global_asm!(
    r#"
    .syntax unified
    .cpu arm1176jzf-s
    .arm
    .section .text.armv6_vectors, "ax"
    .global __conduitos_armv6_vectors_start
__conduitos_armv6_vectors_start:
    ldr pc, [pc, #24]
    ldr pc, [pc, #24]
    ldr pc, [pc, #24]
    ldr pc, [pc, #24]
    ldr pc, [pc, #24]
    ldr pc, [pc, #24]
    ldr pc, [pc, #24]
    ldr pc, [pc, #24]
    .word conduitos_armv6_unexpected_entry
    .word conduitos_armv6_unexpected_entry
    .word conduitos_armv6_unexpected_entry
    .word conduitos_armv6_unexpected_entry
    .word conduitos_armv6_unexpected_entry
    .word conduitos_armv6_unexpected_entry
    .word conduitos_armv6_irq_entry
    .word conduitos_armv6_unexpected_entry
    .global __conduitos_armv6_vectors_end
__conduitos_armv6_vectors_end:

conduitos_armv6_irq_entry:
    stmdb sp!, {{r0-r12, lr}}
    bl conduitos_armv6_irq_handler
    ldmia sp!, {{r0-r12, lr}}
    subs pc, lr, #4

conduitos_armv6_unexpected_entry:
    ldr sp, =__conduitos_exception_stack_end
    mrs r0, cpsr
    mov r1, lr
    mrc p15, 0, r2, c5, c0, 0
    mrc p15, 0, r3, c6, c0, 0
    bl conduitos_armv6_unexpected_handler
"#
);

unsafe fn read32(address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

unsafe fn write32(address: usize, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) };
}

unsafe fn delay(iterations: usize) {
    for _ in 0..iterations {
        unsafe { core::arch::asm!("nop", options(nomem, nostack)) };
    }
}

fn present_hex(value: u32) {
    let mut bytes = [0_u8; 8];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = b"0123456789abcdef"[((value >> ((7 - index) * 4)) & 0xf) as usize];
    }
    present(&bytes);
}

const _: usize = FACT_CAPACITY;
