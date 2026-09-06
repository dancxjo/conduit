#![no_std]
#![no_main]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("conduitos-aarch64-a2 is only an AArch64 machine-wake proof");

use core::panic::PanicInfo;

use conduit_kernel::scheduler::SchedulerStatus;
use conduitos::{
    arch, boot,
    cooperative_timer_lane::{AdmittedLane, LANE_ID},
};

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_a2_start() -> ! {
    arch::enable_fp_simd();
    let mut lane = match AdmittedLane::new() {
        Ok(lane) => lane,
        Err(_) => exit(false),
    };
    let nonce = arch::read_counter();
    entry_sign(nonce);
    let (l0_virtual, l1_virtual, l2_virtual) = arch::mmio_table_addresses();
    let Some(l0_physical) = boot::executable_physical_address(l0_virtual) else {
        exit(false)
    };
    let Some(l1_physical) = boot::executable_physical_address(l1_virtual) else {
        exit(false)
    };
    let Some(l2_physical) = boot::executable_physical_address(l2_virtual) else {
        exit(false)
    };
    arch::install_low_mmio_map(l0_physical, l1_physical, l2_physical);
    arch::initialize_machine();
    stage("machine-init");

    stage("lane-admission");
    stage("lane-handoff");
    if !matches!(lane.step(), Ok(SchedulerStatus::Progress { .. })) {
        refuse("kernel-did-not-request-timer");
    }
    let interest = match lane.take_timer_interest() {
        Ok(interest) => interest,
        Err(_) => refuse("missing-exact-timer-interest"),
    };

    arch::timer_arm();
    stage("idle");
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
            Some(arch::InterruptFact::WrongSource(_)) => refuse("wrong-wake-source"),
            Some(arch::InterruptFact::Overflow) => refuse("interrupt-fact-capacity-exhausted"),
            None => continue,
        }
    }
    arch::disable_interrupts();
    stage("timer-wake");
    if lane.complete_timer(interest).is_err() {
        refuse("stale-timer-identity");
    }
    if !matches!(lane.step(), Ok(SchedulerStatus::Complete)) || lane.pending() != 0 {
        refuse("kernel-terminal-progress-absent");
    }
    machine_sign(nonce, &lane, idle_entries, timer_wakes);
    exit(true)
}

fn entry_sign(nonce: u64) {
    let mut output = Output::new();
    output.push(b"CONDUIT_AARCH64_ENTRY_SIGN {\"schema\":\"conduit.conduitos.aarch64-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"aarch64\",\"build_id\":\"");
    output.push(BUILD_ID.as_bytes());
    output.push(b"\",\"image_id\":\"");
    output.push(IMAGE_ID.as_bytes());
    output.push(b"\",\"bootloader\":\"Limine 12.5.2/BOOTAA64.EFI\",\"emulator_profile\":\"qemu-virt-single-cpu-256m-uefi-semihosting\",\"host_id\":\"host-aarch64-");
    output.hex(nonce.rotate_left(17) ^ 0x434f_4e44_5549_544f);
    output.push(b"\",\"boot_id\":\"boot-aarch64-");
    output.hex(nonce ^ 0x4152_4348_3634_0001);
    output.push(b"\"}\n");
    semihost(0x04, output.bytes().as_ptr() as usize);
}

fn machine_sign(nonce: u64, lane: &AdmittedLane, idle_entries: u32, timer_wakes: u32) {
    let mut output = Output::new();
    output.push(b"CONDUIT_AARCH64_MACHINE_SIGN {\"schema\":\"conduit.conduitos.aarch64-a2-sign/v1\",\"status\":\"completed\",\"architecture\":\"aarch64\",\"boot_id\":\"boot-aarch64-");
    output.hex(nonce ^ 0x4152_4348_3634_0001);
    output.push(b"\",\"lane_id\":\"");
    output.push(LANE_ID.as_bytes());
    output.push(b"\",\"lane_count\":1,\"timer_slots\":1,\"interrupt_fact_slots\":1,\"wake_source\":\"arm-generic-virtual-timer-ppi-27\",\"wake_irq\":27,\"idle_entries\":");
    output.decimal(idle_entries);
    output.push(b",\"timer_wakes\":");
    output.decimal(timer_wakes);
    output.push(b",\"kernel_decisions\":");
    output.decimal(lane.decisions());
    output.push(b",\"kernel_signs\":");
    output.decimal(u32::from(lane.signs()));
    output.push(b",\"pending_host_operations\":0,\"sequence\":[\"machine-init\",\"lane-handoff\",\"idle\",\"timer-wake\",\"terminal\"],\"a3_ordinary_form_claimed\":false}\n");
    arch::present(output.bytes());
}

fn stage(name: &str) {
    arch::present(b"CONDUIT_AARCH64_STAGE ");
    arch::present(name.as_bytes());
    arch::present(b"\n");
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_AARCH64_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    exit(false)
}

struct Output {
    bytes: [u8; 1024],
    len: usize,
}

impl Output {
    const fn new() -> Self {
        Self {
            bytes: [0; 1024],
            len: 0,
        }
    }
    fn push(&mut self, value: &[u8]) {
        let end = self.len + value.len();
        if end > self.bytes.len() {
            refuse("sign-capacity-exhausted");
        }
        self.bytes[self.len..end].copy_from_slice(value);
        self.len = end;
    }
    fn hex(&mut self, value: u64) {
        let mut bytes = [0_u8; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = b"0123456789abcdef"[((value >> ((15 - index) * 4)) & 0xf) as usize];
        }
        self.push(&bytes);
    }
    fn decimal(&mut self, value: u32) {
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
        self.push(&digits[cursor..]);
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn exit(success: bool) -> ! {
    if success {
        unsafe { core::arch::asm!("hvc #0", in("x0") 0x8400_0008_u64, options(noreturn)) }
    }
    loop {
        core::hint::spin_loop();
    }
}

fn semihost(operation: usize, parameter: usize) -> usize {
    let result;
    unsafe {
        core::arch::asm!(
            "hlt #0xf000",
            inout("x0") operation => result,
            in("x1") parameter,
            options(nostack)
        );
    }
    result
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
