//! Only the image's terminal boundary crosses into its supervisor.

pub fn finish(status: u32) -> ! {
    unsafe {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        core::arch::asm!("int 0x80", in("eax") status, options(noreturn));
        #[cfg(target_arch = "aarch64")]
        core::arch::asm!("svc #0", in("x0") u64::from(status), options(noreturn));
        #[cfg(target_arch = "riscv64")]
        core::arch::asm!("ecall", in("a0") status as usize, options(noreturn));
        #[cfg(target_arch = "loongarch64")]
        core::arch::asm!("syscall 0", in("$a0") u64::from(status), options(noreturn));
    }
}
