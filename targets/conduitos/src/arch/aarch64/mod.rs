//! QEMU `virt` AArch64 machine mechanisms below the generic Host seam.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

mod providers;
pub use providers::{Clock, Idle, Interrupts, Serial, Timer};

const UART_BASE: usize = 0x0900_0000;
const GICD_BASE: usize = 0x0800_0000;
const GICC_BASE: usize = 0x0801_0000;
pub const VIRTUAL_TIMER_IRQ: u32 = 27;
const FACT_CAPACITY: u32 = 1;

static FACT_PRESENT: AtomicBool = AtomicBool::new(false);
static FACT_IRQ: AtomicU32 = AtomicU32::new(0);
static FACT_OVERFLOW: AtomicBool = AtomicBool::new(false);

#[repr(C, align(4096))]
struct TranslationTable(UnsafeCell<[u64; 512]>);

unsafe impl Sync for TranslationTable {}

static LOW_L0: TranslationTable = TranslationTable(UnsafeCell::new([0; 512]));
static LOW_L1: TranslationTable = TranslationTable(UnsafeCell::new([0; 512]));
static LOW_L2: TranslationTable = TranslationTable(UnsafeCell::new([0; 512]));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptFact {
    Timer,
    WrongSource(u32),
    Overflow,
}

pub fn enable_fp_simd() {
    let mut cpacr: u64;
    unsafe {
        core::arch::asm!("mrs {cpacr}, cpacr_el1", cpacr = out(reg) cpacr, options(nostack));
        cpacr |= 0b11 << 20;
        core::arch::asm!("msr cpacr_el1, {cpacr}", "isb", cpacr = in(reg) cpacr, options(nostack));
    }
}

pub fn mmio_table_addresses() -> (u64, u64, u64) {
    (
        LOW_L0.0.get() as u64,
        LOW_L1.0.get() as u64,
        LOW_L2.0.get() as u64,
    )
}

pub fn install_low_mmio_map(l0_physical: u64, l1_physical: u64, l2_physical: u64) {
    unsafe {
        (*LOW_L0.0.get())[0] = (l1_physical & 0x0000_ffff_ffff_f000) | 0b11;
        (*LOW_L1.0.get())[0] = (l2_physical & 0x0000_ffff_ffff_f000) | 0b11;
        let device = |physical: u64| {
            (physical & 0x0000_ffff_ffe0_0000) | 1 | (1 << 10) | (7 << 2) | (1 << 53) | (1 << 54)
        };
        (*LOW_L2.0.get())[GICD_BASE >> 21] = device(GICD_BASE as u64);
        (*LOW_L2.0.get())[UART_BASE >> 21] = device(UART_BASE as u64);
        let mut mair: u64;
        core::arch::asm!("mrs {mair}, mair_el1", mair = out(reg) mair, options(nostack));
        mair &= !(0xff_u64 << 56);
        core::arch::asm!("msr mair_el1, {mair}", mair = in(reg) mair, options(nostack));
        let mut tcr: u64;
        core::arch::asm!("mrs {tcr}, tcr_el1", tcr = out(reg) tcr, options(nostack));
        tcr = (tcr & !0x3f & !(1 << 7)) | 16;
        core::arch::asm!(
            "msr ttbr0_el1, {l0}",
            "msr tcr_el1, {tcr}",
            "dsb sy",
            "tlbi vmalle1",
            "dsb sy",
            "isb",
            l0 = in(reg) l0_physical,
            tcr = in(reg) tcr,
            options(nostack)
        );
    }
}

pub fn initialize_machine() {
    disable_interrupts();
    unsafe {
        if current_el() == 2 {
            core::arch::asm!("msr vbar_el2, {vectors}", vectors = in(reg) vectors::address(), options(nostack));
        } else {
            core::arch::asm!("msr vbar_el1, {vectors}", vectors = in(reg) vectors::address(), options(nostack));
        }
        present(b"CONDUIT_AARCH64_MACHINE_DETAIL vectors\n");
        write32(GICD_BASE + 0x000, 0);
        write8(GICD_BASE + 0x400 + VIRTUAL_TIMER_IRQ as usize, 0x80);
        write32(GICD_BASE + 0x100, 1 << VIRTUAL_TIMER_IRQ);
        write32(GICC_BASE + 0x004, 0xff);
        write32(GICC_BASE + 0x000, 0x1);
        write32(GICD_BASE + 0x000, 0x1);
        present(b"CONDUIT_AARCH64_MACHINE_DETAIL gic\n");
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

pub fn timer_arm() {
    let ticks = (counter_frequency() / 1_000).max(1);
    unsafe {
        core::arch::asm!("msr cntv_tval_el0, {ticks:x}", ticks = in(reg) ticks, options(nostack));
        core::arch::asm!("msr cntv_ctl_el0, {enabled:x}", enabled = in(reg) 1_u64, options(nostack));
        core::arch::asm!("isb", options(nostack, preserves_flags));
    }
}

pub fn enable_interrupts() {
    unsafe { core::arch::asm!("msr daifclr, #2", "isb", options(nostack, preserves_flags)) }
}

pub fn disable_interrupts() {
    unsafe { core::arch::asm!("msr daifset, #2", "isb", options(nostack, preserves_flags)) }
}

pub fn interruptible_idle() {
    unsafe { core::arch::asm!("wfi", options(nostack, preserves_flags)) }
}

pub fn pop_interrupt() -> Option<InterruptFact> {
    if FACT_OVERFLOW.swap(false, Ordering::AcqRel) {
        return Some(InterruptFact::Overflow);
    }
    if !FACT_PRESENT.swap(false, Ordering::AcqRel) {
        return None;
    }
    let irq = FACT_IRQ.load(Ordering::Acquire);
    Some(if irq == VIRTUAL_TIMER_IRQ {
        InterruptFact::Timer
    } else {
        InterruptFact::WrongSource(irq)
    })
}

pub fn present(bytes: &[u8]) {
    for byte in bytes {
        while unsafe { read32(UART_BASE + 0x18) } & (1 << 5) != 0 {}
        unsafe { write32(UART_BASE, u32::from(*byte)) }
    }
}

pub fn read_counter() -> u64 {
    let value;
    unsafe {
        core::arch::asm!("mrs {value}, cntvct_el0", value = out(reg) value, options(nostack))
    };
    value
}

pub fn interrupts_enabled() -> bool {
    let value: u64;
    unsafe { core::arch::asm!("mrs {value}, daif", value = out(reg) value, options(nostack)) };
    value & (1 << 7) == 0
}

fn counter_frequency() -> u64 {
    let value;
    unsafe {
        core::arch::asm!("mrs {value}, cntfrq_el0", value = out(reg) value, options(nostack))
    };
    value
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_aarch64_irq_handler() {
    let acknowledge = unsafe { read32(GICC_BASE + 0x00c) };
    unsafe {
        core::arch::asm!("msr cntv_ctl_el0, {disabled:x}", disabled = in(reg) 0_u64, options(nostack));
    }
    let irq = acknowledge & 0x3ff;
    if irq < 1020 {
        if FACT_PRESENT.swap(true, Ordering::AcqRel) {
            FACT_OVERFLOW.store(true, Ordering::Release);
        } else {
            FACT_IRQ.store(irq, Ordering::Release);
        }
        unsafe { write32(GICC_BASE + 0x010, acknowledge) };
    } else {
        FACT_IRQ.store(irq, Ordering::Release);
        FACT_PRESENT.store(true, Ordering::Release);
    }
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_aarch64_exception_handler() -> ! {
    present(b"CONDUIT_AARCH64_REFUSAL unexpected-exception\n");
    let esr: u64;
    let far: u64;
    unsafe {
        if current_el() == 2 {
            core::arch::asm!("mrs {esr}, esr_el2", esr = out(reg) esr, options(nostack));
            core::arch::asm!("mrs {far}, far_el2", far = out(reg) far, options(nostack));
        } else {
            core::arch::asm!("mrs {esr}, esr_el1", esr = out(reg) esr, options(nostack));
            core::arch::asm!("mrs {far}, far_el1", far = out(reg) far, options(nostack));
        }
    }
    present(b"CONDUIT_AARCH64_EXCEPTION_ESR ");
    present_hex(esr);
    present(b" FAR ");
    present_hex(far);
    present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}

fn current_el() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("mrs {value}, CurrentEL", value = out(reg) value, options(nostack)) };
    (value >> 2) & 0x3
}

fn present_hex(value: u64) {
    let mut bytes = [0_u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = b"0123456789abcdef"[((value >> ((15 - index) * 4)) & 0xf) as usize];
    }
    present(&bytes);
}

unsafe fn read32(address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

unsafe fn write32(address: usize, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) }
}

unsafe fn write8(address: usize, value: u8) {
    unsafe { core::ptr::write_volatile(address as *mut u8, value) }
}

mod vectors {
    use core::arch::global_asm;

    global_asm!(
        r#"
        .section .text.aarch64_vectors,"ax"
        .balign 2048
        .global conduitos_aarch64_vectors
conduitos_aarch64_vectors:
        .rept 1
        b conduitos_aarch64_unexpected
        .balign 128
        .endr
        b conduitos_aarch64_irq_entry
        .balign 128
        .rept 14
        b conduitos_aarch64_unexpected
        .balign 128
        .endr
conduitos_aarch64_irq_entry:
        msr tpidr_el1, x18
        adrp x18, __conduitos_irq_stack_top
        add x18, x18, :lo12:__conduitos_irq_stack_top
        mov sp, x18
        sub sp, sp, #256
        stp x0, x1, [sp, #0]
        stp x2, x3, [sp, #16]
        stp x4, x5, [sp, #32]
        stp x6, x7, [sp, #48]
        stp x8, x9, [sp, #64]
        stp x10, x11, [sp, #80]
        stp x12, x13, [sp, #96]
        stp x14, x15, [sp, #112]
        stp x16, x17, [sp, #128]
        stp x19, x20, [sp, #144]
        stp x21, x22, [sp, #160]
        stp x23, x24, [sp, #176]
        stp x25, x26, [sp, #192]
        stp x27, x28, [sp, #208]
        stp x29, x30, [sp, #224]
        bl conduitos_aarch64_irq_handler
        ldp x0, x1, [sp, #0]
        ldp x2, x3, [sp, #16]
        ldp x4, x5, [sp, #32]
        ldp x6, x7, [sp, #48]
        ldp x8, x9, [sp, #64]
        ldp x10, x11, [sp, #80]
        ldp x12, x13, [sp, #96]
        ldp x14, x15, [sp, #112]
        ldp x16, x17, [sp, #128]
        ldp x19, x20, [sp, #144]
        ldp x21, x22, [sp, #160]
        ldp x23, x24, [sp, #176]
        ldp x25, x26, [sp, #192]
        ldp x27, x28, [sp, #208]
        ldp x29, x30, [sp, #224]
        mrs x18, tpidr_el1
        eret
conduitos_aarch64_unexpected:
        bl conduitos_aarch64_exception_handler
        "#
    );

    unsafe extern "C" {
        static conduitos_aarch64_vectors: u8;
    }

    pub fn address() -> usize {
        core::ptr::addr_of!(conduitos_aarch64_vectors) as usize
    }
}

const _: u32 = FACT_CAPACITY;
