#![no_std]
#![no_main]

#[cfg(not(target_arch = "arm"))]
compile_error!("the Raspberry Pi B+ A2 artifact requires an ARM target");

use core::{arch::global_asm, panic::PanicInfo};

use conduit_kernel::scheduler::SchedulerStatus;
use conduitos::{
    arch,
    cooperative_timer_lane::{AdmittedLane, LANE_ID},
};

global_asm!(
    r#"
    .syntax unified
    .cpu arm1176jzf-s
    .arm
    .section .text.entry, "ax"
    .global conduitos_armv6_rpi_b_plus_machine_entry
conduitos_armv6_rpi_b_plus_machine_entry:
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
    bl conduitos_armv6_rpi_b_plus_a2_start
1:
    wfe
    b 1b

    .section .bss.irq_stack, "aw", %nobits
    .balign 16
    .space 4096
__conduitos_irq_stack_end:
    .section .bss.boot_stack, "aw", %nobits
    .balign 16
    .space 8192
__conduitos_boot_stack_end:
    .section .bss.exception_stack, "aw", %nobits
    .balign 16
    .space 4096
    .global __conduitos_exception_stack_end
__conduitos_exception_stack_end:
"#
);

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_armv6_rpi_b_plus_a2_start() -> ! {
    arch::initialize_machine();
    arch::present(b"CONDUIT_ARMV6_RPI_ENTRY_SIGN {\"schema\":\"conduit.conduitos.armv6-rpi-entry/v1\",\"status\":\"entered\",\"architecture\":\"armv6\",\"machine\":\"BCM2835/ARM1176JZF-S\",\"board_target\":\"raspberry-pi-model-b-plus-v1.2\",\"boot_mechanism\":\"direct-kernel\",\"runtime_bases_available\":true}\n");
    let mut lane = AdmittedLane::new().unwrap_or_else(|_| refuse("lane-admission-failed"));
    if !matches!(lane.step(), Ok(SchedulerStatus::Progress { .. })) {
        refuse("kernel-did-not-request-timer");
    }
    let interest = lane
        .take_timer_interest()
        .unwrap_or_else(|_| refuse("missing-exact-timer-interest"));
    arch::timer_arm();
    arch::enable_interrupts();
    let mut idle_entries = 0_u32;
    let mut timer_wakes = 0_u32;
    loop {
        idle_entries = idle_entries.saturating_add(1);
        arch::interruptible_idle();
        match arch::pop_interrupt() {
            Some(arch::InterruptFact::Timer) => {
                timer_wakes = timer_wakes.saturating_add(1);
                break;
            }
            Some(arch::InterruptFact::WrongSource | arch::InterruptFact::Overflow) => {
                refuse("wrong-or-overflowed-wake")
            }
            None => {}
        }
    }
    arch::disable_interrupts();
    lane.complete_timer(interest)
        .unwrap_or_else(|_| refuse("stale-timer-identity"));
    if !matches!(lane.step(), Ok(SchedulerStatus::Complete)) || lane.pending() != 0 {
        refuse("kernel-terminal-progress-absent");
    }
    arch::present(b"CONDUIT_ARMV6_RPI_MACHINE_SIGN {\"schema\":\"conduit.conduitos.armv6-rpi-a2/v1\",\"status\":\"completed\",\"architecture\":\"armv6\",\"machine\":\"BCM2835/ARM1176JZF-S\",\"lane_id\":\"");
    arch::present(LANE_ID.as_bytes());
    arch::present(b"\",\"lane_count\":1,\"timer_slots\":1,\"interrupt_fact_slots\":1,\"wake_source\":\"bcm2835-system-timer-compare-1\",\"wake_irq\":1,\"idle_entries\":");
    decimal(idle_entries);
    arch::present(b",\"timer_wakes\":");
    decimal(timer_wakes);
    arch::present(b",\"pending_host_operations\":0,\"a3_ordinary_form_claimed\":false}\n");
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}

fn decimal(value: u32) {
    let mut digits = [0_u8; 10];
    let mut cursor = digits.len();
    let mut remaining = value;
    loop {
        cursor -= 1;
        digits[cursor] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    arch::present(&digits[cursor..]);
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_ARMV6_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
