//! Independent user boundary checks; absent from the normal product image.
pub use crate::probe_gate::mutate_gate;
use crate::{frame::TextFrame, gate};
pub unsafe fn run(frame: &TextFrame) -> ! {
    unsafe {
        match frame.probe {
            1 => {
                core::arch::asm!("ld t0, 0({0})", in(reg) frame.target, out("t0") _, options(nostack))
            }
            2 => core::arch::asm!("sd zero, 0({0})", in(reg) frame.target, options(nostack)),
            3 => core::arch::asm!("jalr {0}", in(reg) frame.target, clobber_abi("C")),
            4 => core::arch::asm!("csrr t0, satp", out("t0") _, options(nostack)),
            5 => core::arch::asm!("csrci sstatus, 2", options(nostack)),
            6 => loop {
                core::arch::asm!("nop", options(nomem, nostack));
            },
            7 | 17 => floating(frame.probe == 17),
            8 => core::arch::asm!("sret", options(noreturn)),
            9 => core::arch::asm!("mret", options(noreturn)),
            10 => core::arch::asm!("ecall", in("a0") 0_usize, in("a6") 0_usize,
                in("a7") 0x5449_4d45_usize, options(nostack)),
            11 => core::arch::asm!("ebreak", options(nostack)),
            12 => core::arch::asm!("rdtime t0", out("t0") _, options(nostack)),
            13 => core::arch::asm!("ecall", in("a0") 0_usize, in("a7") 99_usize, options(nostack)),
            14 => core::arch::asm!("csrw sie, zero", options(nostack)),
            15 => core::arch::asm!("csrrw t0, 0x015, zero", out("t0") _, options(nostack)),
            16 => core::arch::asm!("2: wfi", "j 2b", options(noreturn)),
            _ => {}
        }
    }
    gate::finish(3)
}
unsafe fn floating(looping: bool) -> ! {
    unsafe {
        core::arch::asm!(r#"
            .option push
            .option arch, +d
            li t0, 0x5a5a5a5a5a5a5a5a
            .irp n,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
                fmv.d.x f\n, t0
            .endr
            li t0, 0xff
            csrw fcsr, t0
            bnez a0, 2f
            li a7, 0
            ecall
            2: nop
            j 2b
            .option pop
        "#, in("a0") u64::from(looping), options(noreturn));
    }
}
