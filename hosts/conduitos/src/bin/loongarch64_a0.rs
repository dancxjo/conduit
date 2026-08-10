#![no_std]
#![no_main]

#[cfg(not(target_arch = "loongarch64"))]
compile_error!("conduitos-loongarch64-a0 must compile as LoongArch64");

use core::panic::PanicInfo;

#[used]
static PROFILE: [u8; 32] = *b"conduitos/loongarch64-a0-elf64@1";
static mut ENTRY_STATE: u64 = 0;

/// A0 establishes a genuine freestanding LoongArch64 ELF entry without a boot claim.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_loongarch64_a0_start() -> ! {
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
