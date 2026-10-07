//! Independent EL0 boundary checks, absent from the ordinary product image.
pub use crate::probe_gate::mutate_gate;
use crate::{frame::TextFrame, gate};

pub unsafe fn run(frame: &TextFrame) -> ! {
    unsafe {
        match frame.probe {
            1 => core::arch::asm!("ldr x9, [{0}]", in(reg) frame.target, out("x9") _, options(nostack)),
            2 => core::arch::asm!("str xzr, [{0}]", in(reg) frame.target, options(nostack)),
            3 => core::arch::asm!("blr {0}", in(reg) frame.target, clobber_abi("C")),
            4 => core::arch::asm!("mrs x9, ttbr0_el1", out("x9") _, options(nostack)),
            5 => core::arch::asm!("msr daifset, #2", options(nostack)),
            6 => loop {
                core::arch::asm!("yield", options(nomem, nostack));
            },
            7 | 17 => poison_floating_and_finish(frame.probe == 17),
            8 => core::arch::asm!("eret", options(noreturn)),
            9 => core::arch::asm!("hvc #0", options(nostack)),
            10 => core::arch::asm!("smc #0", options(nostack)),
            11 => core::arch::asm!("brk #0", options(nostack)),
            12 => core::arch::asm!("mrs x9, cntvct_el0", out("x9") _, options(nostack)),
            13 => core::arch::asm!("svc #1", options(nostack)),
            14 => core::arch::asm!("msr cntp_ctl_el0, xzr", options(nostack)),
            _ => {}
        }
    }
    gate::finish(3)
}

unsafe fn poison_floating_and_finish(looping: bool) -> ! {
    unsafe {
        core::arch::asm!(
            r#"
            .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
                movi v\n\().16b, #0x5a
            .endr
            mov x9, #0x03c00000
            msr fpcr, x9
            mov x9, #0x08000000
            orr x9, x9, #0x1f
            msr fpsr, x9
            cbnz x0, 2f
            svc #0
            2: yield
            b 2b
            "#,
            in("x0") u64::from(looping), options(noreturn),
        );
    }
}
