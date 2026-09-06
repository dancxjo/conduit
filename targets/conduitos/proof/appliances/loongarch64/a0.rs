#![no_std]
#![no_main]

#[cfg(not(target_arch = "loongarch64"))]
compile_error!("conduitos-loongarch64-a0 must compile as LoongArch64");

use core::panic::PanicInfo;

#[used]
static PROFILE: [u8; 32] = *b"conduitos/loongarch64-a0-elf64@1";
static mut ENTRY_STATE: u64 = 0;
const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

/// A0 establishes a genuine freestanding LoongArch64 ELF entry without a boot claim.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_loongarch64_a0_start() -> ! {
    let nonce = read_counter();
    unsafe {
        let first = core::ptr::read_volatile(PROFILE.as_ptr());
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ENTRY_STATE), first.into());
    }
    entry_sign(nonce);
    loop {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) };
    }
}

fn read_counter() -> u64 {
    let value: i64;
    let timer_id: isize;
    unsafe {
        core::arch::asm!(
            "rdtime.d {}, {}",
            out(reg) value,
            out(reg) timer_id,
            options(readonly, nostack)
        );
    }
    let _ = timer_id;
    value as u64
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
    present(output.bytes());
}

fn present(bytes: &[u8]) {
    const UART: *mut u8 = 0x1fe0_01e0 as *mut u8;
    for &byte in bytes {
        unsafe { core::ptr::write_volatile(UART, byte) };
    }
}

struct Output {
    bytes: [u8; 768],
    len: usize,
}

impl Output {
    const fn new() -> Self {
        Self {
            bytes: [0; 768],
            len: 0,
        }
    }
    fn push(&mut self, value: &[u8]) {
        let end = self.len + value.len();
        if end > self.bytes.len() {
            loop {
                core::hint::spin_loop();
            }
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                value.as_ptr(),
                self.bytes.as_mut_ptr().add(self.len),
                value.len(),
            );
        }
        self.len = end;
    }
    fn hex(&mut self, value: u64) {
        let mut bytes = [0_u8; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = unsafe {
                *b"0123456789abcdef".get_unchecked(((value >> ((15 - index) * 4)) & 0xf) as usize)
            };
        }
        self.push(&bytes);
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(destination: *mut u8, source: *const u8, count: usize) -> *mut u8 {
    for index in 0..count {
        unsafe { destination.add(index).write(source.add(index).read()) };
    }
    destination
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memset(destination: *mut u8, value: i32, count: usize) -> *mut u8 {
    for index in 0..count {
        unsafe { destination.add(index).write(value as u8) };
    }
    destination
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
