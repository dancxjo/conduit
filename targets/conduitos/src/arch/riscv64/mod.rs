//! QEMU `virt` RISC-V64 mechanisms below the generic machine seam.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

mod providers;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer};

const SBI_EXT_TIME: usize = 0x5449_4d45;
const SUPERVISOR_TIMER_INTERRUPT: usize = 5;
const SIE_STIE: usize = 1 << SUPERVISOR_TIMER_INTERRUPT;
const SSTATUS_SIE: usize = 1 << 1;

static FACT_PRESENT: AtomicBool = AtomicBool::new(false);
static FACT_CAUSE: AtomicU64 = AtomicU64::new(0);
static FACT_OVERFLOW: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptFact {
    Timer,
    WrongSource(u64),
    Overflow,
}

core::arch::global_asm!(
    r#"
    .align 4
    .global conduitos_riscv64_trap_vector
conduitos_riscv64_trap_vector:
    addi sp, sp, -128
    sd ra,   0(sp)
    sd t0,   8(sp)
    sd t1,  16(sp)
    sd t2,  24(sp)
    sd a0,  32(sp)
    sd a1,  40(sp)
    sd a2,  48(sp)
    sd a3,  56(sp)
    sd a4,  64(sp)
    sd a5,  72(sp)
    sd a6,  80(sp)
    sd a7,  88(sp)
    sd t3,  96(sp)
    sd t4, 104(sp)
    sd t5, 112(sp)
    sd t6, 120(sp)
    call conduitos_riscv64_trap_handler
    ld ra,   0(sp)
    ld t0,   8(sp)
    ld t1,  16(sp)
    ld t2,  24(sp)
    ld a0,  32(sp)
    ld a1,  40(sp)
    ld a2,  48(sp)
    ld a3,  56(sp)
    ld a4,  64(sp)
    ld a5,  72(sp)
    ld a6,  80(sp)
    ld a7,  88(sp)
    ld t3,  96(sp)
    ld t4, 104(sp)
    ld t5, 112(sp)
    ld t6, 120(sp)
    addi sp, sp, 128
    sret
"#
);

unsafe extern "C" {
    static conduitos_riscv64_trap_vector: u8;
}

pub fn initialize_machine() -> bool {
    disable_interrupts();
    FACT_PRESENT.store(false, Ordering::Release);
    FACT_OVERFLOW.store(false, Ordering::Release);
    unsafe {
        core::arch::asm!("csrw stvec, {0}", in(reg) &conduitos_riscv64_trap_vector, options(nostack));
        core::arch::asm!("csrs sie, {0}", in(reg) SIE_STIE, options(nostack));
    }
    let stvec: usize;
    let sie: usize;
    unsafe {
        core::arch::asm!("csrr {0}, stvec", out(reg) stvec, options(nostack));
        core::arch::asm!("csrr {0}, sie", out(reg) sie, options(nostack));
    }
    stvec & !0b11 == unsafe { &conduitos_riscv64_trap_vector as *const u8 as usize }
        && sie & SIE_STIE != 0
}

pub fn timer_arm() -> bool {
    let deadline = read_counter().saturating_add(100_000);
    let error: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") deadline => error,
            lateout("a1") _,
            in("a6") 0_usize,
            in("a7") SBI_EXT_TIME,
            options(nostack)
        );
    }
    error == 0
}

pub fn enable_interrupts() {
    unsafe { core::arch::asm!("csrs sstatus, {0}", in(reg) SSTATUS_SIE, options(nostack)) }
}

pub fn disable_interrupts() {
    unsafe { core::arch::asm!("csrc sstatus, {0}", in(reg) SSTATUS_SIE, options(nostack)) }
}

pub fn interrupts_enabled() -> bool {
    let status: usize;
    unsafe { core::arch::asm!("csrr {0}, sstatus", out(reg) status, options(nostack)) };
    status & SSTATUS_SIE != 0
}

pub fn interruptible_idle() {
    unsafe { core::arch::asm!("wfi", options(nostack)) }
}

pub fn pop_interrupt() -> Option<InterruptFact> {
    if FACT_OVERFLOW.swap(false, Ordering::AcqRel) {
        return Some(InterruptFact::Overflow);
    }
    if !FACT_PRESENT.swap(false, Ordering::AcqRel) {
        return None;
    }
    let cause = FACT_CAUSE.load(Ordering::Acquire);
    let timer = (1_u64 << 63) | SUPERVISOR_TIMER_INTERRUPT as u64;
    Some(if cause == timer {
        InterruptFact::Timer
    } else {
        InterruptFact::WrongSource(cause)
    })
}

pub fn present(bytes: &[u8]) {
    for byte in bytes {
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") usize::from(*byte),
                in("a7") 1_usize,
                options(nostack)
            );
        }
    }
}

pub fn read_counter() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("rdtime {0}", out(reg) value, options(nostack)) };
    value
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_riscv64_trap_handler() {
    let cause: u64;
    unsafe {
        core::arch::asm!("csrr {0}, scause", out(reg) cause, options(nostack));
        core::arch::asm!("csrc sie, {0}", in(reg) SIE_STIE, options(nostack));
    }
    if FACT_PRESENT.swap(true, Ordering::AcqRel) {
        FACT_OVERFLOW.store(true, Ordering::Release);
    } else {
        FACT_CAUSE.store(cause, Ordering::Release);
    }
}
