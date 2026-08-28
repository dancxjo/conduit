#![no_std]
#![no_main]

#[cfg(not(target_arch = "arm"))]
compile_error!("the Raspberry Pi B+ A0 artifact requires an ARM target");

use core::{arch::global_asm, panic::PanicInfo, ptr};

const GPIO_BASE: usize = 0x2020_0000;
const UART0_BASE: usize = 0x2020_1000;

global_asm!(
    r#"
    .syntax unified
    .cpu arm1176jzf-s
    .arm
    .section .text.entry, "ax"
    .global conduitos_armv6_rpi_b_plus_entry
    .type conduitos_armv6_rpi_b_plus_entry, %function
conduitos_armv6_rpi_b_plus_entry:
    cpsid if
    ldr sp, =__conduitos_boot_stack_end
    ldr r0, =__bss_start
    ldr r1, =__bss_end
    mov r2, #0
0:
    cmp r0, r1
    strlo r2, [r0], #4
    blo 0b
    bl conduitos_armv6_rpi_b_plus_a0_start
1:
    wfe
    b 1b
    .size conduitos_armv6_rpi_b_plus_entry, . - conduitos_armv6_rpi_b_plus_entry

    .section .bss.boot_stack, "aw", %nobits
    .balign 16
__conduitos_boot_stack:
    .space 4096
__conduitos_boot_stack_end:
"#
);

/// The direct entry emits an early machine-relative UART marker. This is useful
/// for emulator diagnosis and future attached-board acceptance, but the A0
/// build record deliberately makes no boot or runtime-Base claim.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_armv6_rpi_b_plus_a0_start() -> ! {
    initialize_pl011();
    write_serial(b"CONDUIT_ARMV6_RPI_ENTRY_SIGN {\"schema\":\"conduit.conduitos.armv6-rpi-entry/v1\",\"status\":\"entered\",\"architecture\":\"armv6\",\"machine\":\"BCM2835/ARM1176JZF-S\",\"board_target\":\"raspberry-pi-model-b-plus-v1.2\",\"boot_mechanism\":\"direct-kernel\",\"runtime_bases_available\":false}\n");
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}

fn initialize_pl011() {
    unsafe {
        write(UART0_BASE + 0x30, 0);
        let mut function = read(GPIO_BASE + 0x04);
        function &= !((7 << 12) | (7 << 15));
        function |= (4 << 12) | (4 << 15);
        write(GPIO_BASE + 0x04, function);
        write(GPIO_BASE + 0x94, 0);
        delay(150);
        write(GPIO_BASE + 0x98, (1 << 14) | (1 << 15));
        delay(150);
        write(GPIO_BASE + 0x98, 0);
        write(UART0_BASE + 0x24, 1);
        write(UART0_BASE + 0x28, 40);
        write(UART0_BASE + 0x2c, (1 << 4) | (3 << 5));
        write(UART0_BASE + 0x38, 0);
        write(UART0_BASE + 0x44, 0x7ff);
        write(UART0_BASE + 0x30, 1 | (1 << 8) | (1 << 9));
    }
}

fn write_serial(bytes: &[u8]) {
    for &byte in bytes {
        while unsafe { read(UART0_BASE + 0x18) } & (1 << 5) != 0 {
            core::hint::spin_loop();
        }
        unsafe { write(UART0_BASE, u32::from(byte)) };
    }
}

unsafe fn read(address: usize) -> u32 {
    unsafe { ptr::read_volatile(address as *const u32) }
}

unsafe fn write(address: usize, value: u32) {
    unsafe { ptr::write_volatile(address as *mut u32, value) };
}

unsafe fn delay(iterations: usize) {
    for _ in 0..iterations {
        unsafe { core::arch::asm!("nop", options(nomem, nostack)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
