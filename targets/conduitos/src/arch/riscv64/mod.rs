//! QEMU `virt` RISC-V64 mechanisms below the generic machine seam.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

mod providers;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer};
mod entropy;
pub use entropy::SeedEntropy;
mod domain_budget;
#[cfg(feature = "riscv64-product")]
mod domain_memory;
#[cfg(feature = "riscv64-product")]
mod domain_transition;
#[cfg(feature = "riscv64-product")]
#[path = "../ordinary_domain.rs"]
mod ordinary_domain;
#[cfg(feature = "riscv64-product")]
pub use ordinary_domain::TextDomain;
#[cfg(feature = "riscv64-product")]
pub fn initialize_domains(record: &crate::boot::BootRecord) {
    domain_memory::initialize(record);
}
#[cfg(feature = "riscv64-product")]
fn domain_ticks() -> u64 {
    read_counter()
}
pub fn early_write(bytes: &[u8]) {
    present(bytes);
}
#[cfg(feature = "ordinary-domain-proof")]
pub fn start_pending_source_timer() {
    providers::start_pending_source_timer();
}

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
    .option push
    .option arch, +d
    .align 4
    .global conduitos_riscv64_trap_vector
conduitos_riscv64_trap_vector:
    addi sp, sp, -400
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
    fsd f0, 128(sp)
    fsd f1, 136(sp)
    fsd f2, 144(sp)
    fsd f3, 152(sp)
    fsd f4, 160(sp)
    fsd f5, 168(sp)
    fsd f6, 176(sp)
    fsd f7, 184(sp)
    fsd f8, 192(sp)
    fsd f9, 200(sp)
    fsd f10, 208(sp)
    fsd f11, 216(sp)
    fsd f12, 224(sp)
    fsd f13, 232(sp)
    fsd f14, 240(sp)
    fsd f15, 248(sp)
    fsd f16, 256(sp)
    fsd f17, 264(sp)
    fsd f18, 272(sp)
    fsd f19, 280(sp)
    fsd f20, 288(sp)
    fsd f21, 296(sp)
    fsd f22, 304(sp)
    fsd f23, 312(sp)
    fsd f24, 320(sp)
    fsd f25, 328(sp)
    fsd f26, 336(sp)
    fsd f27, 344(sp)
    fsd f28, 352(sp)
    fsd f29, 360(sp)
    fsd f30, 368(sp)
    fsd f31, 376(sp)
    frcsr t0
    sd t0, 384(sp)
    call conduitos_riscv64_trap_handler
    fld f0, 128(sp)
    fld f1, 136(sp)
    fld f2, 144(sp)
    fld f3, 152(sp)
    fld f4, 160(sp)
    fld f5, 168(sp)
    fld f6, 176(sp)
    fld f7, 184(sp)
    fld f8, 192(sp)
    fld f9, 200(sp)
    fld f10, 208(sp)
    fld f11, 216(sp)
    fld f12, 224(sp)
    fld f13, 232(sp)
    fld f14, 240(sp)
    fld f15, 248(sp)
    fld f16, 256(sp)
    fld f17, 264(sp)
    fld f18, 272(sp)
    fld f19, 280(sp)
    fld f20, 288(sp)
    fld f21, 296(sp)
    fld f22, 304(sp)
    fld f23, 312(sp)
    fld f24, 320(sp)
    fld f25, 328(sp)
    fld f26, 336(sp)
    fld f27, 344(sp)
    fld f28, 352(sp)
    fld f29, 360(sp)
    fld f30, 368(sp)
    fld f31, 376(sp)
    ld t0, 384(sp)
    fscsr t0
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
    addi sp, sp, 400
    sret
    .option pop
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
        core::arch::asm!("csrs sstatus, {0}", in(reg) 0x6000_usize, options(nostack));
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
    domain_budget::source_arm()
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

pub const fn emergency_machine_profile() -> crate::machine::EmergencyMachineProfile {
    crate::machine::EmergencyMachineProfile {
        halt: crate::machine::EmergencyMachineAvailability::Available,
        reset: crate::machine::EmergencyMachineAvailability::Unavailable,
    }
}

pub fn emergency_halt() -> ! {
    disable_interrupts();
    loop {
        unsafe { core::arch::asm!("wfi", options(nostack)) }
    }
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
    }
    if cause == (1_u64 << 63) | SUPERVISOR_TIMER_INTERRUPT as u64 {
        domain_budget::interrupt(false);
        return;
    }
    if FACT_PRESENT.swap(true, Ordering::AcqRel) {
        FACT_OVERFLOW.store(true, Ordering::Release);
    } else {
        FACT_CAUSE.store(cause, Ordering::Release);
    }
}
fn record_timer_interrupt() {
    if FACT_PRESENT.swap(true, Ordering::AcqRel) {
        FACT_OVERFLOW.store(true, Ordering::Release);
    } else {
        FACT_CAUSE.store(
            (1_u64 << 63) | SUPERVISOR_TIMER_INTERRUPT as u64,
            Ordering::Release,
        );
    }
}
