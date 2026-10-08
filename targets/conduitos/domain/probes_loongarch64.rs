//! Independent PLV3 checks; omitted from the ordinary product image.
pub use crate::probe_gate::mutate_gate;
use crate::{frame::TextFrame, gate};
pub unsafe fn run(frame: &TextFrame) -> ! {
    unsafe {
        match frame.probe {
            1 => {
                core::arch::asm!("ld.d $t0, {0}, 0",in(reg)frame.target,out("$t0") _,options(nostack))
            }
            2 => core::arch::asm!("st.d $zero, {0}, 0",in(reg)frame.target,options(nostack)),
            3 => core::arch::asm!("jirl $ra, {0}, 0",in(reg)frame.target,clobber_abi("C")),
            4 => core::arch::asm!("csrrd $t0, 0x19",out("$t0") _,options(nostack)),
            5 => core::arch::asm!("csrwr $zero, 0", options(nostack)),
            6 => loop {
                core::arch::asm!("nop", options(nomem, nostack));
            },
            7 | 17 => floating(frame.probe == 17),
            8 => core::arch::asm!("ertn", options(noreturn)),
            9 => core::arch::asm!("iocsrrd.w $t0, $zero",out("$t0") _,options(nostack)),
            11 => core::arch::asm!("break 0", options(nostack)),
            12 => core::arch::asm!("rdtime.d $t0, $t1",out("$t0") _,out("$t1") _,options(nostack)),
            13 => core::arch::asm!("syscall 99",in("$a0")0u64,options(nostack)),
            14 => core::arch::asm!("csrwr $zero, 0x41", options(nostack)),
            16 => core::arch::asm!("2: idle 0", "b 2b", options(noreturn)),
            18 => core::arch::asm!("move $sp, $zero","syscall 0",in("$a0")0u64,options(noreturn)),
            19 => core::arch::asm!("move $sp, $zero", "2: nop", "b 2b", options(noreturn)),
            _ => {}
        }
    }
    gate::finish(3)
}
unsafe fn floating(looping: bool) -> ! {
    unsafe {
        core::arch::asm!(r#"
        li.d $t0, 0x5a5a5a5a5a5a5a5a
        movgr2fr.d $f0, $t0
        movgr2fr.d $f1, $t0
        movgr2fr.d $f2, $t0
        movgr2fr.d $f3, $t0
        movgr2fr.d $f4, $t0
        movgr2fr.d $f5, $t0
        movgr2fr.d $f6, $t0
        movgr2fr.d $f7, $t0
        movgr2fr.d $f8, $t0
        movgr2fr.d $f9, $t0
        movgr2fr.d $f10, $t0
        movgr2fr.d $f11, $t0
        movgr2fr.d $f12, $t0
        movgr2fr.d $f13, $t0
        movgr2fr.d $f14, $t0
        movgr2fr.d $f15, $t0
        movgr2fr.d $f16, $t0
        movgr2fr.d $f17, $t0
        movgr2fr.d $f18, $t0
        movgr2fr.d $f19, $t0
        movgr2fr.d $f20, $t0
        movgr2fr.d $f21, $t0
        movgr2fr.d $f22, $t0
        movgr2fr.d $f23, $t0
        movgr2fr.d $f24, $t0
        movgr2fr.d $f25, $t0
        movgr2fr.d $f26, $t0
        movgr2fr.d $f27, $t0
        movgr2fr.d $f28, $t0
        movgr2fr.d $f29, $t0
        movgr2fr.d $f30, $t0
        movgr2fr.d $f31, $t0
        li.d $t0, 0x300
        movgr2fcsr $fcsr0, $t0
        li.d $t0, 1
        movgr2cf $fcc0, $t0
        movgr2cf $fcc1, $t0
        movgr2cf $fcc2, $t0
        movgr2cf $fcc3, $t0
        movgr2cf $fcc4, $t0
        movgr2cf $fcc5, $t0
        movgr2cf $fcc6, $t0
        movgr2cf $fcc7, $t0
        bnez $a0, 2f
        syscall 0
        2: nop
        b 2b
    "#,in("$a0")u64::from(looping),options(noreturn));
    }
}
