#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[cfg(not(target_arch = "arm"))]
compile_error!("the Mini PiTFT image requires the ARMv6 Raspberry Pi target");

use conduitos::{allocation::BOOT_ARENA, arch};
use core::{
    arch::global_asm,
    cell::UnsafeCell,
    panic::PanicInfo,
    sync::atomic::{AtomicU8, Ordering},
};

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const ARENA_BYTES: usize = 128 * 1024 * 1024;
#[repr(C, align(64))]
struct Arena(UnsafeCell<[u8; ARENA_BYTES]>);
unsafe impl Sync for Arena {}
#[used]
#[unsafe(link_section = ".bss.runtime_arena")]
static ARENA: Arena = Arena(UnsafeCell::new([0; ARENA_BYTES]));

// IRQs stay disabled and every borrow ends before any application preparation.
// Indicator setters contain no allocation or callbacks that could re-enter it.
struct Indicator(UnsafeCell<Option<arch::boot_indicator::BootIndicator>>);
unsafe impl Sync for Indicator {}
static INDICATOR: Indicator = Indicator(UnsafeCell::new(None));
static STAGE: AtomicU8 = AtomicU8::new(1);

global_asm!(
    r#"
    .syntax unified
    .cpu arm1176jzf-s
    .arm
    .section .text.entry, "ax"
    .global conduitos_armv6_rpi_b_plus_product_entry
conduitos_armv6_rpi_b_plus_product_entry:
    cpsid if
    cps #0x12
    ldr sp, =__conduitos_irq_stack_end
    cps #0x13
    ldr sp, =__conduitos_boot_stack_end
    ldr r0, =__bss_start
    ldr r1, =__bss_end
    mov r2, #0
0:
    cmp r0, r1
    strlo r2, [r0], #4
    blo 0b
    bl conduitos_armv6_minipitft_start
1:
    wfe
    b 1b
    .section .bss.irq_stack, "aw", %nobits
    .balign 16
    .space 4096
__conduitos_irq_stack_end:
    .section .bss.boot_stack, "aw", %nobits
    .balign 16
    .space 524288
__conduitos_boot_stack_end:
    .section .bss.exception_stack, "aw", %nobits
    .balign 16
    .space 4096
    .global __conduitos_exception_stack_end
__conduitos_exception_stack_end:
"#
);

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_armv6_minipitft_start() -> ! {
    arch::initialize_machine();
    // SAFETY: this explicit B+ image claims the onboard ACT LED handed over by
    // firmware, separately from the Mini PiTFT's eight reserved contacts.
    unsafe {
        *INDICATOR.0.get() = arch::boot_indicator::admit().ok();
    }
    stage(1);
    // SAFETY: the linked BSS arena is exclusive, writable and within512MiB RAM.
    unsafe { BOOT_ARENA.initialize(ARENA.0.get() as usize, ARENA_BYTES) }
        .unwrap_or_else(|_| refuse("minipitft-arena-refused"));
    // SAFETY: this exact image selects the native B+ attachment, ordered MMIO,
    // exclusive firmware handoff and fixed250MHz core clock in config.txt.
    let retained = unsafe { conduitos::protocol_boot::minipitft::run(BUILD_ID, stage) }
        .unwrap_or_else(|reason| refuse(reason));
    arch::present(b"CONDUIT_MINIPITFT_DISPLAY_DIAGNOSTIC frame-completed buttons-pending\n");
    loop {
        // The retained successful Play keeps backlight possession. No new
        // controller transaction, capability admission or frame is fabricated.
        core::hint::black_box(&retained);
        led(true);
        wait(100_000);
        led(false);
        wait(1_900_000);
    }
}

fn stage(value: u8) {
    STAGE.store(value, Ordering::Relaxed);
    led(true);
    arch::present(b"CONDUIT_MINIPITFT_STAGE ");
    arch::present(&[b'0' + value]);
    arch::present(b"\n");
}
fn led(on: bool) {
    // SAFETY: the sole Root accesses the retained indicator with IRQs disabled.
    // The short borrow ends before entering any fallible application code.
    if let Some(indicator) = unsafe { (&mut *INDICATOR.0.get()).as_mut() } {
        let _ = indicator.set(on);
    }
}
fn wait(micros: u32) {
    let start = arch::read_counter() as u32;
    // Bounded work even if the physical timer unexpectedly stops.
    for _ in 0..100_000_000 {
        if (arch::read_counter() as u32).wrapping_sub(start) >= micros {
            return;
        }
        core::hint::spin_loop();
    }
}
fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_MINIPITFT_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        for _ in 0..STAGE.load(Ordering::Relaxed).clamp(1, 6) {
            led(true);
            wait(150_000);
            led(false);
            wait(150_000);
        }
        wait(1_200_000);
    }
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
#[alloc_error_handler]
fn allocation_error(_: core::alloc::Layout) -> ! {
    refuse("allocation-capacity-exhausted")
}
