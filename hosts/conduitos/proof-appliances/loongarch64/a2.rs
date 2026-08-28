#![no_std]
#![no_main]

#[cfg(not(target_arch = "loongarch64"))]
compile_error!("conduitos-loongarch64-a2 must compile as LoongArch64");

use core::panic::PanicInfo;

use conduit_kernel::scheduler::SchedulerStatus;
use conduitos::{
    arch,
    cooperative_timer_lane::{AdmittedLane, LANE_ID},
};

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.conduitos_loongarch64_a2_start")]
pub extern "C" fn conduitos_loongarch64_a2_start() -> ! {
    let nonce = arch::read_counter();
    entry_sign(nonce);
    let mut lane = AdmittedLane::new().unwrap_or_else(|_| refuse("base-exhaustion"));
    if !arch::initialize_machine() {
        refuse("unavailable-or-stale-trap-controller");
    }
    stage("machine-init");
    stage("lane-handoff");
    if !matches!(lane.step(), Ok(SchedulerStatus::Progress { .. })) {
        refuse("kernel-did-not-request-timer");
    }
    let interest = lane
        .take_timer_interest()
        .unwrap_or_else(|_| refuse("missing-exact-timer-interest"));
    if !arch::timer_arm() {
        refuse("unavailable-loongarch-timer-mechanism");
    }
    stage("idle");
    arch::enable_interrupts();
    let mut idle_entries = 0_u32;
    loop {
        idle_entries = idle_entries.saturating_add(1);
        arch::interruptible_idle();
        match arch::pop_interrupt() {
            Some(arch::InterruptFact::Timer) => break,
            Some(arch::InterruptFact::WrongSource(_)) => refuse("wrong-wake-source"),
            Some(arch::InterruptFact::Overflow) => refuse("interrupt-fact-capacity-exhausted"),
            None => continue,
        }
    }
    arch::disable_interrupts();
    stage("timer-wake");
    lane.complete_timer(interest)
        .unwrap_or_else(|_| refuse("stale-timer-identity"));
    if !matches!(lane.step(), Ok(SchedulerStatus::Complete)) || lane.pending() != 0 {
        refuse("kernel-terminal-progress-absent");
    }
    machine_sign(nonce, &lane, idle_entries);
    loop {
        core::hint::spin_loop();
    }
}

fn entry_sign(nonce: u64) {
    let mut output = Output::new();
    output.push(b"CONDUIT_LOONGARCH64_ENTRY_SIGN {\"schema\":\"conduit.conduitos.loongarch64-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"loongarch64\",\"build_id\":\"");
    output.push(BUILD_ID.as_bytes());
    output.push(b"\",\"image_id\":\"");
    output.push(IMAGE_ID.as_bytes());
    output.push(b"\",\"bootloader\":\"Limine 12.5.2/BOOTLOONGARCH64.EFI\",\"emulator_profile\":\"qemu-loongarch64-virt-single-cpu-2g-edk2\",\"firmware\":\"EDK2 QEMU_EFI.fd (mechanism only)\",\"host_id\":\"host-loongarch64-");
    output.hex(nonce.rotate_left(17) ^ 0x434f_4e44_5549_5401);
    output.push(b"\",\"boot_id\":\"boot-loongarch64-");
    output.hex(nonce ^ 0x4c41_3634_0000_0001);
    output.push(b"\",\"runtime_bases_available\":false,\"a2_machine_wake_claimed\":false}\n");
    arch::present(output.bytes());
}

fn machine_sign(nonce: u64, lane: &AdmittedLane, idle_entries: u32) {
    let mut output = Output::new();
    output.push(b"CONDUIT_LOONGARCH64_MACHINE_SIGN {\"schema\":\"conduit.conduitos.loongarch64-a2-sign/v1\",\"status\":\"completed\",\"architecture\":\"loongarch64\",\"boot_id\":\"boot-loongarch64-");
    output.hex(nonce ^ 0x4c41_3634_0000_0001);
    output.push(b"\",\"kernel\":\"conduit-kernel\",\"lane_id\":\"");
    output.push(LANE_ID.as_bytes());
    output.push(b"\",\"lane_count\":1,\"runtime_base_count\":4,\"runtime_memory_bytes\":4096,\"timer_slots\":1,\"interrupt_fact_slots\":1,\"wake_source\":\"loongarch-local-timer-interrupt\",\"wake_cause\":11,\"timer_mechanism\":\"TCFG/TICLR\",\"idle_entries\":");
    output.decimal(idle_entries);
    output.push(b",\"timer_wakes\":1,\"kernel_decisions\":");
    output.decimal(lane.decisions());
    output.push(b",\"kernel_signs\":");
    output.decimal(u32::from(lane.signs()));
    output.push(b",\"pending_host_operations\":0,\"sequence\":[\"machine-init\",\"lane-handoff\",\"idle\",\"timer-wake\",\"terminal\"],\"a3_ordinary_form_claimed\":false}\n");
    arch::present(output.bytes());
}

fn stage(name: &str) {
    arch::present(b"CONDUIT_LOONGARCH64_STAGE ");
    arch::present(name.as_bytes());
    arch::present(b"\n");
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_LOONGARCH64_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
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

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
