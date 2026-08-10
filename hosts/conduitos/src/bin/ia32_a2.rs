#![no_std]
#![no_main]

#[cfg(not(target_arch = "x86"))]
compile_error!("conduitos-ia32-a2 must compile as 32-bit x86");

use conduit_kernel::scheduler::SchedulerStatus;
use conduitos::{
    allocation::BootArena,
    arch,
    machine_a2_kernel::{AdmittedLane, LANE_ID},
};
use core::panic::PanicInfo;

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

#[global_allocator]
static BOOT_ARENA: BootArena = BootArena::new();
static mut MEMORY_ARENA: [u8; 4096] = [0; 4096];

#[used]
#[unsafe(link_section = ".multiboot")]
static MULTIBOOT1_HEADER: [u32; 6] = [0x1bad_b002, 4, 0xe452_4ffa, 0, 0, 0];

core::arch::global_asm!(
    r#"
.section .text.conduitos_ia32_a2_start,"ax",@progbits
.global conduitos_ia32_a2_start
.type conduitos_ia32_a2_start,@function
conduitos_ia32_a2_start:
    mov eax, cr0
    and eax, 0xfffffffb
    or eax, 0x2
    mov cr0, eax
    mov eax, cr4
    or eax, 0x600
    mov cr4, eax
    jmp conduitos_ia32_a2_rust_entry
"#
);

#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_a2_rust_entry() -> ! {
    if unsafe {
        BOOT_ARENA.initialize(
            core::ptr::addr_of_mut!(MEMORY_ARENA) as *mut u8 as usize,
            4096,
        )
    }
    .is_err()
    {
        refuse("memory-base-unavailable");
    }
    let mut lane = AdmittedLane::new().unwrap_or_else(|_| refuse("base-exhaustion"));
    if BOOT_ARENA.seal() != 0 || BOOT_ARENA.capacity() != 4096 {
        refuse("memory-base-exhausted-before-play");
    }
    let nonce = arch::read_counter();
    entry_sign(nonce);
    arch::initialize_machine();
    stage("machine-init");
    stage("lane-admission");
    stage("lane-handoff");
    if !matches!(lane.step(), Ok(SchedulerStatus::Progress { .. })) {
        refuse("kernel-did-not-request-timer");
    }
    let interest = lane
        .take_timer_interest()
        .unwrap_or_else(|_| refuse("missing-exact-timer-interest"));
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
                timer_wakes += 1;
                break;
            }
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
    machine_sign(nonce, &lane, idle_entries, timer_wakes);
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) }
    }
}

fn entry_sign(nonce: u64) {
    let mut out = Output::new();
    out.push(b"CONDUIT_IA32_ENTRY_SIGN {\"schema\":\"conduit.conduitos.ia32-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"ia32\",\"build_id\":\"");
    out.push(BUILD_ID.as_bytes());
    out.push(b"\",\"image_id\":\"");
    out.push(IMAGE_ID.as_bytes());
    out.push(b"\",\"bootloader\":\"Limine 12.5.2/BOOTIA32.EFI\",\"emulator_profile\":\"qemu-i386-q35-single-cpu-512m-uefi-debugcon\",\"host_id\":\"host-ia32-");
    out.hex(nonce.rotate_left(17) ^ 0x434f_4e44_5549_544f);
    out.push(b"\",\"boot_id\":\"boot-ia32-");
    out.hex(nonce ^ 0x4941_3332_0000_0001);
    out.push(b"\"}\n");
    arch::present(out.bytes());
}

fn machine_sign(nonce: u64, lane: &AdmittedLane, idle: u32, wakes: u32) {
    let mut out = Output::new();
    out.push(b"CONDUIT_IA32_MACHINE_SIGN {\"schema\":\"conduit.conduitos.ia32-a2-sign/v1\",\"status\":\"completed\",\"architecture\":\"ia32\",\"boot_id\":\"boot-ia32-");
    out.hex(nonce ^ 0x4941_3332_0000_0001);
    out.push(b"\",\"lane_id\":\"");
    out.push(LANE_ID.as_bytes());
    out.push(b"\",\"base_count\":7,\"lane_count\":1,\"memory_arena_bytes\":4096,\"timer_slots\":1,\"interrupt_fact_slots\":1,\"controller\":\"8259-pic-remapped-irq0-vector32\",\"wake_source\":\"8254-pit-channel0-irq0\",\"wake_irq\":32,\"idle_entries\":");
    out.decimal(idle);
    out.push(b",\"timer_wakes\":");
    out.decimal(wakes);
    out.push(b",\"kernel_decisions\":");
    out.decimal(lane.decisions());
    out.push(b",\"kernel_signs\":");
    out.decimal(u32::from(lane.signs()));
    out.push(b",\"pending_host_operations\":0,\"sequence\":[\"machine-init\",\"lane-handoff\",\"idle\",\"timer-wake\",\"terminal\"],\"a3_ordinary_form_claimed\":false}\n");
    arch::present(out.bytes());
}

fn stage(name: &str) {
    arch::present(b"CONDUIT_IA32_STAGE ");
    arch::present(name.as_bytes());
    arch::present(b"\n");
}
fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_IA32_REFUSAL ");
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
        if self.len + value.len() > self.bytes.len() {
            refuse("sign-capacity-exhausted");
        }
        for byte in value {
            unsafe { *self.bytes.get_unchecked_mut(self.len) = *byte };
            self.len += 1;
        }
    }
    fn hex(&mut self, value: u64) {
        for shift in (0..16).rev() {
            self.push(&[b"0123456789abcdef"[((value >> (shift * 4)) & 0xf) as usize]]);
        }
    }
    fn decimal(&mut self, value: u32) {
        let mut digits = [0_u8; 10];
        let mut cursor = 10;
        let mut rest = value;
        loop {
            cursor -= 1;
            digits[cursor] = b'0' + (rest % 10) as u8;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        self.push(&digits[cursor..]);
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, count: usize) -> *mut u8 {
    for i in 0..count {
        unsafe { dst.add(i).write(src.add(i).read()) };
    }
    dst
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(dst: *mut u8, value: i32, count: usize) -> *mut u8 {
    for i in 0..count {
        unsafe { dst.add(i).write(value as u8) };
    }
    dst
}

#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() {}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
