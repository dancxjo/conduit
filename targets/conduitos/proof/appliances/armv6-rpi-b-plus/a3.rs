#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[cfg(not(target_arch = "arm"))]
compile_error!("the Raspberry Pi B+ A3 artifact requires an ARM target");

use core::{arch::global_asm, cell::UnsafeCell, panic::PanicInfo};

use conduitos::{allocation::BOOT_ARENA, arch, boot, dual_region_plan, identity, sign_format};

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");
const BOARD_ID: &str = env!("CONDUITOS_BOARD_ID");
const ARENA_BYTES: usize = 4 * 1024 * 1024;

#[repr(C, align(64))]
struct RuntimeArena(UnsafeCell<[u8; ARENA_BYTES]>);

unsafe impl Sync for RuntimeArena {}

#[used]
#[unsafe(link_section = ".bss.runtime_arena")]
static RUNTIME_ARENA: RuntimeArena = RuntimeArena(UnsafeCell::new([0; ARENA_BYTES]));

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
    bl conduitos_armv6_rpi_b_plus_a3_start
1:
    wfe
    b 1b

    .section .bss.irq_stack, "aw", %nobits
    .balign 16
    .space 4096
__conduitos_irq_stack_end:
    .section .bss.boot_stack, "aw", %nobits
    .balign 16
    .space 262144
__conduitos_boot_stack_end:
    .section .bss.exception_stack, "aw", %nobits
    .balign 16
    .space 4096
    .global __conduitos_exception_stack_end
__conduitos_exception_stack_end:
"#
);

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_armv6_rpi_b_plus_a3_start() -> ! {
    arch::initialize_machine();
    arch::present(b"CONDUIT_ARMV6_RPI_ENTRY_SIGN {\"schema\":\"conduit.conduitos.armv6-rpi-entry/v1\",\"status\":\"entered\",\"architecture\":\"armv6\",\"machine\":\"BCM2835/ARM1176JZF-S\",\"board_target\":\"");
    arch::present(BOARD_ID.as_bytes());
    arch::present(b"\",\"firmware_board_revision\":");
    if let Some(revision) = arch::firmware_board_revision() {
        arch::present(b"\"");
        present_hex(revision);
        arch::present(b"\"");
    } else {
        arch::present(b"null");
    }
    arch::present(b",\"boot_mechanism\":\"direct-kernel\",\"runtime_bases_available\":true}\n");
    let nonce = arch::read_counter();
    let record = boot_record(nonce).unwrap_or_else(|error| refuse(error.as_str()));
    stage("boot-record");
    unsafe {
        BOOT_ARENA
            .initialize(RUNTIME_ARENA.0.get() as usize, ARENA_BYTES)
            .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    }
    stage("arena");
    let identities = identity::derive(
        [
            nonce,
            nonce.rotate_left(7),
            nonce.rotate_left(19),
            nonce.rotate_left(31),
        ],
        record.timestamp,
        record.image_physical_start,
    );
    let offer = conduitos::offer::HostOffer::new(
        &identities,
        BUILD_ID,
        conduitos::offer::CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        record.runtime_arena.length,
    );
    offer
        .validate()
        .unwrap_or_else(|error| refuse(error.as_str()));
    stage("offer");
    let mut prepared = dual_region_plan::prepare(&identities, &offer, BUILD_ID)
        .unwrap_or_else(|error| refuse(error.as_str()));
    stage("plan");
    let allocation_before_play = BOOT_ARENA.seal();
    let mut clock = arch::Clock::new();
    let mut timer = arch::Timer::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    let report = conduitos::dual_region_composition::run(
        &mut prepared.kernel,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    stage("play");
    let sign = sign_format::machine_accepted(
        &identities,
        &offer,
        &report,
        &prepared,
        sign_format::AllocationReceipt {
            before_play: allocation_before_play,
            after_play: BOOT_ARENA.used(),
            capacity: BOOT_ARENA.capacity(),
        },
        BUILD_ID,
    )
    .unwrap_or_else(|_| refuse("kernel-sign-storage-full"));
    arch::present(b"\n");
    arch::present(sign.as_bytes());
    arch::present(b"CONDUIT_ARMV6_RPI_A3_IDENTITY {\"image_id\":\"");
    arch::present(IMAGE_ID.as_bytes());
    arch::present(b"\",\"wake_source\":\"bcm2835-system-timer-compare-1\",\"wake_irq\":1,\"a3_ordinary_form_claimed\":true}\n");
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}

fn stage(name: &str) {
    arch::present(b"CONDUIT_ARMV6_STAGE ");
    arch::present(name.as_bytes());
    arch::present(b"\n");
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

fn present_hex(value: u32) {
    let mut bytes = [0_u8; 8];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = b"0123456789abcdef"[((value >> ((7 - index) * 4)) & 0xf) as usize];
    }
    arch::present(&bytes);
}

fn boot_record(nonce: u64) -> Result<boot::BootRecord, boot::BootError> {
    unsafe extern "C" {
        static __image_start: u8;
        static __image_file_end: u8;
    }
    let image_start = core::ptr::addr_of!(__image_start) as u64;
    let image_end = core::ptr::addr_of!(__image_file_end) as u64;
    let mut normalizer = boot::BootNormalizer::new(
        boot::Firmware::RaspberryPiVideoCore,
        nonce,
        0,
        image_start,
        image_end - image_start,
    )?;
    normalizer.push_region(boot::MemoryRegion {
        base: image_start,
        length: image_end - image_start,
        kind: boot::MemoryKind::ExecutableAndArtifacts,
    })?;
    normalizer.push_region(boot::MemoryRegion {
        base: RUNTIME_ARENA.0.get() as u64,
        length: ARENA_BYTES as u64,
        kind: boot::MemoryKind::Usable,
    })?;
    normalizer.set_command_line(b"")?;
    normalizer.finish()
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

#[alloc_error_handler]
fn allocation_error(layout: core::alloc::Layout) -> ! {
    arch::present(b"CONDUIT_ARMV6_REFUSAL allocation-capacity-exhausted size=");
    decimal(layout.size() as u32);
    arch::present(b" used=");
    decimal(BOOT_ARENA.used() as u32);
    arch::present(b" capacity=");
    decimal(BOOT_ARENA.capacity() as u32);
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}
