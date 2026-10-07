//! Emulator-only hostile implementation probes; omitted from product artifacts.
use crate::{frame::TextFrame, gate};

pub unsafe fn run(frame: &TextFrame) -> ! {
    unsafe {
        match frame.probe {
            1 => core::arch::asm!("mov rax, [{0}]", in(reg) frame.target, out("rax") _, options(nostack)),
            2 => core::arch::asm!("mov byte ptr [{0}], 0xff", in(reg) frame.target, options(nostack)),
            3 => core::arch::asm!("call {0}", in(reg) frame.target),
            4 => core::arch::asm!("out dx, al", in("dx") 0x80u16, in("al") 0u8, options(nostack)),
            5 => core::arch::asm!("cli", options(nostack)),
            6 => loop { core::arch::asm!("pause", options(nomem, nostack)); },
            7 => core::arch::asm!("pxor xmm0, xmm0", options(nostack)),
            _ => {}
        }
    }
    gate::finish(3)
}
