//! Tegra210 UART and architectural timer mechanisms. Firmware owns UART clocks,
//! pinmux and DRAM setup. No USB, GPU, storage or DMA authority is offered here.
//! Register provenance: Linux v6.12 tegra210.dtsi (GIC and UART nodes).
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[path = "../aarch64/providers.rs"]
mod providers;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer};
#[path = "vectors.rs"]
mod vectors;

const UART: usize = 0x7000_6000;
const GICD: usize = 0x5004_1000;
const GICC: usize = 0x5004_2000;
const TIMER_IRQ: u32 = 27;
static SERIAL_FAILED: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
static SOURCE: AtomicU32 = AtomicU32::new(0);
static OVERFLOW: AtomicBool = AtomicBool::new(false);

pub enum InterruptFact {
    Timer,
    WrongSource(u32),
    Overflow,
}

pub fn initialize_machine() {
    disable_interrupts();
    unsafe {
        core::arch::asm!("msr vbar_el1, {0}", "isb", in(reg) vectors::address(), options(nostack));
        // Only the banked virtual-timer PPI is selected; do not change SPI
        // routing belonging to firmware display or other retained devices.
        write32(GICD + 0x180, 1 << TIMER_IRQ);
        write32(GICD + 0x280, 1 << TIMER_IRQ);
        let groups = read32(GICD + 0x80);
        write32(GICD + 0x80, groups | (1 << TIMER_IRQ));
        core::ptr::write_volatile((GICD + 0x400 + TIMER_IRQ as usize) as *mut u8, 0x80);
        write32(GICC + 4, 0xff);
        write32(GICC, 1);
        write32(GICD, read32(GICD) | 1);
        write32(GICD + 0x100, 1 << TIMER_IRQ);
        core::arch::asm!("dsb sy", "isb", options(nostack));
    }
}

pub fn present(bytes: &[u8]) {
    for byte in bytes {
        // Tegra's 16550 registers are 32-bit words, four bytes apart.
        let mut remaining = 1_000_000u32;
        while unsafe { read32(UART + 5 * 4) } & (1 << 5) == 0 {
            if remaining == 0 {
                SERIAL_FAILED.store(true, Ordering::Release);
                return;
            }
            remaining -= 1;
            core::hint::spin_loop();
        }
        unsafe {
            write32(UART, u32::from(*byte));
        }
    }
}

pub fn enable_fp_simd() {
    unsafe {
        let mut cpacr: u64;
        core::arch::asm!("mrs {0}, cpacr_el1", out(reg) cpacr, options(nostack));
        cpacr |= 3 << 20;
        core::arch::asm!("msr cpacr_el1, {0}", "isb", in(reg) cpacr, options(nostack));
    }
}
pub fn enable_interrupts() {
    unsafe {
        core::arch::asm!("msr daifclr, #2", "isb", options(nostack));
    }
}
pub fn disable_interrupts() {
    unsafe {
        core::arch::asm!("msr daifset, #2", "isb", options(nostack));
    }
}
fn interrupts_enabled() -> bool {
    let daif: u64;
    unsafe {
        core::arch::asm!("mrs {0}, daif",out(reg) daif,options(nostack));
    }
    daif & (1 << 7) == 0
}
fn interruptible_idle() {
    unsafe {
        core::arch::asm!("wfi", options(nostack));
    }
}
pub fn read_counter() -> u64 {
    let value;
    unsafe {
        core::arch::asm!("mrs {0}, cntvct_el0",out(reg) value,options(nostack));
    }
    value
}
fn counter_frequency() -> u64 {
    let value;
    unsafe {
        core::arch::asm!("mrs {0}, cntfrq_el0",out(reg) value,options(nostack));
    }
    value
}
fn timer_arm() {
    timer_arm_ticks((counter_frequency() / 1000).clamp(1, i32::MAX as u64));
}
fn timer_arm_ticks(ticks: u64) {
    unsafe {
        core::arch::asm!("msr cntv_tval_el0, {0}","msr cntv_ctl_el0, {1}","isb",in(reg) ticks,in(reg) 1u64,options(nostack));
    }
}
fn pop_interrupt() -> Option<InterruptFact> {
    if OVERFLOW.swap(false, Ordering::AcqRel) {
        return Some(InterruptFact::Overflow);
    }
    if !PENDING.swap(false, Ordering::AcqRel) {
        return None;
    }
    let source = SOURCE.load(Ordering::Acquire);
    Some(if source == TIMER_IRQ {
        InterruptFact::Timer
    } else {
        InterruptFact::WrongSource(source)
    })
}
#[unsafe(no_mangle)]
extern "C" fn conduitos_nano_irq() {
    let acknowledge = unsafe { read32(GICC + 0xc) };
    let source = acknowledge & 0x3ff;
    if source >= 1020 {
        return;
    }
    if source == TIMER_IRQ {
        unsafe {
            core::arch::asm!("msr cntv_ctl_el0, {0}","isb",in(reg) 0u64,options(nostack));
        }
    }
    if PENDING.load(Ordering::Acquire) {
        OVERFLOW.store(true, Ordering::Release);
    } else {
        SOURCE.store(source, Ordering::Relaxed);
        PENDING.store(true, Ordering::Release);
    }
    unsafe {
        write32(GICC + 0x10, acknowledge);
    }
}
#[unsafe(no_mangle)]
extern "C" fn conduitos_nano_exception() -> ! {
    disable_interrupts();
    present(b"CONDUIT_JETSON_NANO_REFUSAL unexpected-exception\n");
    loop {
        unsafe {
            core::arch::asm!("wfe", options(nostack));
        }
    }
}
unsafe fn read32(address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}
unsafe fn write32(address: usize, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) }
}

pub fn serial_healthy() -> bool {
    !SERIAL_FAILED.load(Ordering::Acquire)
}
