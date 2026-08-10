#![no_std]
#![no_main]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("conduitos-aarch64-a0 is only an AArch64 compile/link artifact");

use core::cell::UnsafeCell;
use core::panic::PanicInfo;

#[repr(C)]
struct BaseRevision(UnsafeCell<[u64; 3]>);

unsafe impl Sync for BaseRevision {}

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static REQUESTS_START: [u64; 4] = [
    0xf6b8_f4b3_9de7_d1ae,
    0xfab9_1a69_40fc_b9cf,
    0x785c_6ed0_15d3_e316,
    0x181e_920a_7852_b9d9,
];

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision(UnsafeCell::new([
    0xf956_2b2d_5c95_a6c8,
    0x6a7b_3849_4453_6bdc,
    6,
]));

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static REQUESTS_END: [u64; 2] = [0xadc0_e053_1bb1_0d03, 0x9572_709f_3176_4c62];

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

/// The accepted A0 entry now supplies the deliberately tiny A1 proof path.
/// Semihosting output and the PSCI test exit are emulator proof mechanisms
/// only; neither is a runtime Base or available to later proof rungs.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_a0_start() -> ! {
    let nonce = counter();
    emit_sign(nonce);
    exit(true)
}

fn counter() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("mrs {value}, cntvct_el0", value = out(reg) value) };
    value
}

fn emit_sign(nonce: u64) {
    let mut buffer = [0_u8; 512];
    let mut length = 0;
    append(&mut buffer, &mut length, b"CONDUIT_AARCH64_ENTRY_SIGN {\"schema\":\"conduit.conduitos.aarch64-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"aarch64\",\"build_id\":\"");
    append(&mut buffer, &mut length, BUILD_ID.as_bytes());
    append(&mut buffer, &mut length, b"\",\"image_id\":\"");
    append(&mut buffer, &mut length, IMAGE_ID.as_bytes());
    append(&mut buffer, &mut length, b"\",\"bootloader\":\"Limine 12.5.2/BOOTAA64.EFI\",\"emulator_profile\":\"qemu-virt-single-cpu-256m-uefi-semihosting\",\"host_id\":\"host-aarch64-");
    append_hex(
        &mut buffer,
        &mut length,
        nonce.rotate_left(17) ^ 0x434f_4e44_5549_544f,
    );
    append(&mut buffer, &mut length, b"\",\"boot_id\":\"boot-aarch64-");
    append_hex(&mut buffer, &mut length, nonce ^ 0x4152_4348_3634_0001);
    append(&mut buffer, &mut length, b"\"}\n");
    semihost(0x04, buffer.as_ptr() as usize);
}

fn append(buffer: &mut [u8; 512], length: &mut usize, value: &[u8]) {
    let end = *length + value.len();
    buffer[*length..end].copy_from_slice(value);
    *length = end;
}

fn append_hex(buffer: &mut [u8; 512], length: &mut usize, value: u64) {
    let mut bytes = [0_u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let shift = (15 - index) * 4;
        *byte = b"0123456789abcdef"[((value >> shift) & 0xf) as usize];
    }
    append(buffer, length, &bytes);
}

fn exit(success: bool) -> ! {
    if success {
        unsafe {
            core::arch::asm!("hvc #0", in("x0") 0x8400_0008_u64, options(noreturn));
        }
    }
    loop {
        core::hint::spin_loop();
    }
}

fn semihost(operation: usize, parameter: usize) -> usize {
    let result: usize;
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
    loop {
        core::hint::spin_loop();
    }
}
