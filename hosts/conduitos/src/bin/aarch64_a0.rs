#![no_std]
#![no_main]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("conduitos-aarch64-a0 is only an AArch64 compile/link artifact");

use core::panic::PanicInfo;

/// A0 establishes only the AArch64 entry ABI and a linked freestanding image.
/// It intentionally does not claim that an instruction has executed.
#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_a0_start() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
