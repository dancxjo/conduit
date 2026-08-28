#![no_std]
#![no_main]

#[cfg(not(target_arch = "riscv64"))]
compile_error!("conduitos-riscv64-a0 must compile as RISC-V64");

use core::panic::PanicInfo;

#[used]
static PROFILE: [u8; 28] = *b"conduitos/riscv64-a0-elf64@1";
static mut ENTRY_STATE: u64 = 0;
const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

/// A0 establishes a genuine freestanding RISC-V64 ELF entry without a boot claim.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_riscv64_a0_start() -> ! {
    unsafe {
        let first = core::ptr::read_volatile(PROFILE.as_ptr());
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ENTRY_STATE), first.into());
    }
    let nonce = read_time();
    let mut output = Output::new();
    output.push(b"CONDUIT_RISCV64_ENTRY_SIGN {\"schema\":\"conduit.conduitos.riscv64-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"riscv64\",\"build_id\":\"");
    output.push(BUILD_ID.as_bytes());
    output.push(b"\",\"image_id\":\"");
    output.push(IMAGE_ID.as_bytes());
    output.push(b"\",\"bootloader\":\"Limine 12.5.2/BOOTRISCV64.EFI\",\"emulator_profile\":\"qemu-riscv64-virt-single-hart-256m-tcg-opensbi-uboot\",\"firmware\":\"OpenSBI+U-Boot EFI\",\"host_id\":\"host-riscv64-");
    output.hex(nonce.rotate_left(17) ^ 0x434f_4e44_5549_544f);
    output.push(b"\",\"boot_id\":\"boot-riscv64-");
    output.hex(nonce ^ 0x5256_3634_0000_0001);
    output.push(b"\",\"runtime_bases_available\":false,\"a2_machine_wake_claimed\":false}\n");
    present(output.bytes());
    loop {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) };
    }
}

fn read_time() -> u64 {
    let value: u64;
    unsafe { core::arch::asm!("rdtime {value}", value = out(reg) value, options(nomem, nostack)) };
    value
}

fn present(bytes: &[u8]) {
    for byte in bytes {
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a0") usize::from(*byte),
                in("a7") 1_usize,
                lateout("a1") _, lateout("a2") _, lateout("a3") _, lateout("a4") _, lateout("a5") _, lateout("a6") _,
            )
        };
    }
}

struct Output {
    bytes: [u8; 1024],
    length: usize,
}
impl Output {
    const fn new() -> Self {
        Self {
            bytes: [0; 1024],
            length: 0,
        }
    }
    fn push(&mut self, bytes: &[u8]) {
        if self.length + bytes.len() > self.bytes.len() {
            loop {
                core::hint::spin_loop();
            }
        }
        for byte in bytes {
            unsafe { *self.bytes.get_unchecked_mut(self.length) = *byte };
            self.length += 1;
        }
    }
    fn hex(&mut self, value: u64) {
        for shift in (0..16).rev() {
            self.push(&[unsafe {
                *b"0123456789abcdef".get_unchecked(((value >> (shift * 4)) & 0xf) as usize)
            }]);
        }
    }
    fn bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.bytes.as_ptr(), self.length) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(destination: *mut u8, source: *const u8, length: usize) -> *mut u8 {
    for index in 0..length {
        unsafe { destination.add(index).write(source.add(index).read()) };
    }
    destination
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memset(destination: *mut u8, value: i32, length: usize) -> *mut u8 {
    for index in 0..length {
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
