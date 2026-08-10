#![no_std]
#![no_main]

#[cfg(not(target_arch = "x86"))]
compile_error!("conduitos-ia32-a0 must compile as 32-bit x86");

use core::panic::PanicInfo;

core::arch::global_asm!(
    r#"
.section .text.conduitos_ia32_a0_start,"ax",@progbits
.global conduitos_ia32_a0_start
.type conduitos_ia32_a0_start,@function
conduitos_ia32_a0_start:
    mov eax, cr0
    and eax, 0xfffffffb
    or eax, 0x2
    mov cr0, eax
    mov eax, cr4
    or eax, 0x600
    mov cr4, eax
    jmp conduitos_ia32_a1_rust_entry
"#
);

#[used]
static PROFILE: [u8; 25] = *b"conduitos/ia32-a0-elf32@1";
static mut ENTRY_STATE: u32 = 0;
const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

#[used]
#[unsafe(link_section = ".multiboot")]
static MULTIBOOT1_HEADER: [u32; 6] = [0x1bad_b002, 4, 0xe452_4ffa, 0, 0, 0];

/// A0 establishes the IA-32 entry ABI and a genuine freestanding ELF32 image.
/// It deliberately makes no claim that this instruction stream has executed.
#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_a1_rust_entry() -> ! {
    let nonce = counter();
    unsafe {
        let first = core::ptr::read_volatile(PROFILE.as_ptr());
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ENTRY_STATE), first.into());
    }
    emit_sign(nonce);
    exit()
}

fn counter() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)) };
    u64::from(low) | (u64::from(high) << 32)
}

fn emit_sign(nonce: u64) {
    let mut buffer = [0_u8; 512];
    let mut length = 0;
    append(&mut buffer, &mut length, b"CONDUIT_IA32_ENTRY_SIGN {\"schema\":\"conduit.conduitos.ia32-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"ia32\",\"build_id\":\"");
    append(&mut buffer, &mut length, BUILD_ID.as_bytes());
    append(&mut buffer, &mut length, b"\",\"image_id\":\"");
    append(&mut buffer, &mut length, IMAGE_ID.as_bytes());
    append(&mut buffer, &mut length, b"\",\"bootloader\":\"Limine 12.5.2/BOOTIA32.EFI\",\"emulator_profile\":\"qemu-i386-q35-single-cpu-512m-uefi-debugcon\",\"host_id\":\"host-ia32-");
    append_hex(
        &mut buffer,
        &mut length,
        nonce.rotate_left(17) ^ 0x434f_4e44_5549_544f,
    );
    append(&mut buffer, &mut length, b"\",\"boot_id\":\"boot-ia32-");
    append_hex(&mut buffer, &mut length, nonce ^ 0x4941_3332_0000_0001);
    append(&mut buffer, &mut length, b"\"}\n");
    for index in 0..length {
        outb(0xe9, unsafe { *buffer.get_unchecked(index) });
    }
}

fn append(buffer: &mut [u8; 512], length: &mut usize, value: &[u8]) {
    for byte in value {
        unsafe { *buffer.get_unchecked_mut(*length) = *byte };
        *length += 1;
    }
}

fn append_hex(buffer: &mut [u8; 512], length: &mut usize, value: u64) {
    let mut bytes = [0_u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let shift = (15 - index) * 4;
        *byte = unsafe { *b"0123456789abcdef".get_unchecked(((value >> shift) & 0xf) as usize) };
    }
    append(buffer, length, &bytes);
}

fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags))
    };
}

fn exit() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(destination: *mut u8, source: *const u8, count: usize) -> *mut u8 {
    for offset in 0..count {
        unsafe { destination.add(offset).write(source.add(offset).read()) };
    }
    destination
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memset(destination: *mut u8, value: i32, count: usize) -> *mut u8 {
    for offset in 0..count {
        unsafe { destination.add(offset).write(value as u8) };
    }
    destination
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
