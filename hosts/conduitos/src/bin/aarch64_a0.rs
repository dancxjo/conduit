#![no_std]
#![no_main]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("conduitos-aarch64-a0 is only an AArch64 compile/link artifact");

use core::panic::PanicInfo;

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

/// The accepted A0 entry now supplies the deliberately tiny A1 proof path.
/// Semihosting is an emulator test-exit mechanism only; it is not a runtime
/// Base and must not be used by later ConduitOS proof rungs.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_a0_start() -> ! {
    let nonce = counter();
    write0(
        "CONDUIT_AARCH64_ENTRY_SIGN {\"schema\":\"conduit.conduitos.aarch64-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"aarch64\",\"build_id\":\"",
    );
    write0(BUILD_ID);
    write0("\",\"image_id\":\"");
    write0(IMAGE_ID);
    write0(
        "\",\"bootloader\":\"Limine 12.5.2/BOOTAA64.EFI\",\"emulator_profile\":\"qemu-virt-single-cpu-64m-uefi-semihosting\",\"host_id\":\"host-aarch64-",
    );
    write_hex(nonce.rotate_left(17) ^ 0x434f_4e44_5549_544f);
    write0("\",\"boot_id\":\"boot-aarch64-");
    write_hex(nonce ^ 0x4152_4348_3634_0001);
    write0("\"}\n");
    exit(true)
}

fn counter() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("mrs {value}, cntvct_el0", value = out(reg) value) };
    value
}

fn write_hex(value: u64) {
    let mut bytes = [0_u8; 17];
    for (index, byte) in bytes[..16].iter_mut().enumerate() {
        let shift = (15 - index) * 4;
        *byte = b"0123456789abcdef"[((value >> shift) & 0xf) as usize];
    }
    write0(unsafe { core::str::from_utf8_unchecked(&bytes) });
}

fn write0(text: &str) {
    let bytes = text.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() {
        let length = core::cmp::min(bytes.len() - offset, 255);
        let mut buffer = [0_u8; 256];
        buffer[..length].copy_from_slice(&bytes[offset..offset + length]);
        semihost(0x04, buffer.as_ptr() as usize);
        offset += length;
    }
}

fn exit(success: bool) -> ! {
    let block = [0x20026_usize, usize::from(!success)];
    semihost(0x20, block.as_ptr() as usize);
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
