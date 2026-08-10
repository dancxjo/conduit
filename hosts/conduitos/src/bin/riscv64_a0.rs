#![no_std]
#![no_main]

#[cfg(not(target_arch = "riscv64"))]
compile_error!("conduitos-riscv64-a0 must compile as RISC-V64");

use core::panic::PanicInfo;

#[used]
static PROFILE: [u8; 28] = *b"conduitos/riscv64-a0-elf64@1";
static mut ENTRY_STATE: u64 = 0;

/// A0 establishes a genuine freestanding RISC-V64 ELF entry without a boot claim.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_riscv64_a0_start() -> ! {
    unsafe {
        let first = core::ptr::read_volatile(PROFILE.as_ptr());
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ENTRY_STATE), first.into());
    }
    loop {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
