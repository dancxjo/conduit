//! QEMU `virt` LoongArch64 mechanisms below the generic machine seam.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

mod providers;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer};

const CRMD_IE: usize = 1 << 2;
const TIMER_INTERRUPT: usize = 11;
const ECFG_TIMER: usize = 1 << TIMER_INTERRUPT;
const TCFG_ENABLE: usize = 1;
const UART: *mut u8 = 0x1fe0_01e0 as *mut u8;

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
    .align 12
    .global conduitos_loongarch64_trap_vector
conduitos_loongarch64_trap_vector:
    addi.d $r3, $r3, -160
    st.d $r1,  $r3,   0
    st.d $r4,  $r3,   8
    st.d $r5,  $r3,  16
    st.d $r6,  $r3,  24
    st.d $r7,  $r3,  32
    st.d $r8,  $r3,  40
    st.d $r9,  $r3,  48
    st.d $r10, $r3,  56
    st.d $r11, $r3,  64
    st.d $r12, $r3,  72
    st.d $r13, $r3,  80
    st.d $r14, $r3,  88
    st.d $r15, $r3,  96
    st.d $r16, $r3, 104
    st.d $r17, $r3, 112
    st.d $r18, $r3, 120
    st.d $r19, $r3, 128
    st.d $r20, $r3, 136
    bl conduitos_loongarch64_trap_handler
    ld.d $r1,  $r3,   0
    ld.d $r4,  $r3,   8
    ld.d $r5,  $r3,  16
    ld.d $r6,  $r3,  24
    ld.d $r7,  $r3,  32
    ld.d $r8,  $r3,  40
    ld.d $r9,  $r3,  48
    ld.d $r10, $r3,  56
    ld.d $r11, $r3,  64
    ld.d $r12, $r3,  72
    ld.d $r13, $r3,  80
    ld.d $r14, $r3,  88
    ld.d $r15, $r3,  96
    ld.d $r16, $r3, 104
    ld.d $r17, $r3, 112
    ld.d $r18, $r3, 120
    ld.d $r19, $r3, 128
    ld.d $r20, $r3, 136
    addi.d $r3, $r3, 160
    ertn
"#
);

unsafe extern "C" {
    static conduitos_loongarch64_trap_vector: u8;
}

fn read_csr<const CSR: u32>() -> usize {
    let value: usize;
    unsafe {
        core::arch::asm!("csrrd {value}, {csr}", value = out(reg) value, csr = const CSR, options(nostack))
    };
    value
}
fn write_csr<const CSR: u32>(value: usize) {
    let mut scratch = value;
    unsafe {
        core::arch::asm!("csrwr {value}, {csr}", value = inout(reg) scratch, csr = const CSR, options(nostack))
    };
    let _ = scratch;
}
fn change_csr<const CSR: u32>(value: usize, mask: usize) {
    let mut scratch = value;
    unsafe {
        core::arch::asm!("csrxchg {value}, {mask}, {csr}", value = inout(reg) scratch, mask = in(reg) mask, csr = const CSR, options(nostack))
    };
    let _ = scratch;
}

pub fn initialize_machine() -> bool {
    disable_interrupts();
    FACT_PRESENT.store(false, Ordering::Release);
    FACT_OVERFLOW.store(false, Ordering::Release);
    write_csr::<0x0c>(unsafe { &conduitos_loongarch64_trap_vector as *const u8 as usize });
    change_csr::<0x04>(ECFG_TIMER, ECFG_TIMER | (0b111 << 16));
    read_csr::<0x0c>() == unsafe { &conduitos_loongarch64_trap_vector as *const u8 as usize }
        && read_csr::<0x04>() & ECFG_TIMER != 0
}

pub fn timer_arm() -> bool {
    write_csr::<0x44>(1);
    write_csr::<0x41>((100_000 << 2) | TCFG_ENABLE);
    read_csr::<0x41>() & TCFG_ENABLE != 0
}
pub fn enable_interrupts() {
    change_csr::<0x00>(CRMD_IE, CRMD_IE);
}
pub fn disable_interrupts() {
    change_csr::<0x00>(0, CRMD_IE);
}
pub fn interrupts_enabled() -> bool {
    read_csr::<0x00>() & CRMD_IE != 0
}
pub fn interruptible_idle() {
    unsafe { core::arch::asm!("idle 0", options(nostack)) }
}

pub fn pop_interrupt() -> Option<InterruptFact> {
    if FACT_OVERFLOW.swap(false, Ordering::AcqRel) {
        return Some(InterruptFact::Overflow);
    }
    if !FACT_PRESENT.swap(false, Ordering::AcqRel) {
        return None;
    }
    let cause = FACT_CAUSE.load(Ordering::Acquire);
    Some(if cause == TIMER_INTERRUPT as u64 {
        InterruptFact::Timer
    } else {
        InterruptFact::WrongSource(cause)
    })
}
pub fn present(bytes: &[u8]) {
    for &byte in bytes {
        unsafe { core::ptr::write_volatile(UART, byte) };
    }
}
pub fn read_counter() -> u64 {
    let value: i64;
    let timer_id: isize;
    unsafe {
        core::arch::asm!("rdtime.d {}, {}", out(reg) value, out(reg) timer_id, options(readonly, nostack))
    };
    let _ = timer_id;
    value as u64
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_loongarch64_trap_handler() {
    let status = read_csr::<0x05>();
    let cause = (status & 0x1fff).trailing_zeros() as u64;
    write_csr::<0x44>(1);
    change_csr::<0x04>(0, ECFG_TIMER);
    if FACT_PRESENT.swap(true, Ordering::AcqRel) {
        FACT_OVERFLOW.store(true, Ordering::Release);
    } else {
        FACT_CAUSE.store(cause, Ordering::Release);
    }
}
