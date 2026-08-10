#![no_std]
#![no_main]

#[cfg(not(target_arch = "x86"))]
compile_error!("conduitos-ia32-a0 must compile as 32-bit x86");

use core::panic::PanicInfo;

#[used]
static PROFILE: [u8; 25] = *b"conduitos/ia32-a0-elf32@1";
static mut ENTRY_STATE: u32 = 0;

/// A0 establishes the IA-32 entry ABI and a genuine freestanding ELF32 image.
/// It deliberately makes no claim that this instruction stream has executed.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_ia32_a0_start() -> ! {
    unsafe {
        let first = core::ptr::read_volatile(PROFILE.as_ptr());
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ENTRY_STATE), first.into());
    }
    loop {
        unsafe { core::arch::asm!("pause", options(nomem, nostack, preserves_flags)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
