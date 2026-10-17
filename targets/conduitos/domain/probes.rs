//! Emulator-only hostile implementation probes; omitted from product artifacts.
use crate::{frame::TextFrame, gate};

pub use crate::probe_gate::mutate_gate;

pub unsafe fn run(frame: &TextFrame) -> ! {
    unsafe {
        match frame.probe {
            1 => {
                core::arch::asm!("mov rax, [{0}]", in(reg) frame.target, out("rax") _, options(nostack))
            }
            2 => {
                core::arch::asm!("mov byte ptr [{0}], 0xff", in(reg) frame.target, options(nostack))
            }
            3 => core::arch::asm!("call {0}", in(reg) frame.target),
            4 => core::arch::asm!("out dx, al", in("dx") 0x80u16, in("al") 0u8, options(nostack)),
            5 => core::arch::asm!("cli", options(nostack)),
            6 => loop {
                core::arch::asm!("pause", options(nomem, nostack));
            },
            7 => core::arch::asm!("pxor xmm0, xmm0", options(nostack)),
            8 => core::arch::asm!("syscall", options(nostack)),
            9 => core::arch::asm!("sysenter", options(nostack)),
            10 => {
                core::arch::asm!("xor ecx, ecx", "xor edx, edx", "div rcx", out("rax") _, out("rcx") _, out("rdx") _, options(nostack))
            }
            11 => core::arch::asm!("int3", options(nostack)),
            12 => core::arch::asm!("pushfq", "or qword ptr [rsp], 0x100", "popfq", "nop"),
            14 => core::arch::asm!("rdtsc", out("eax") _, out("edx") _, options(nostack)),
            17 => {
                core::arch::asm!("std", options(nostack));
                loop {
                    core::arch::asm!("pause", options(nomem, nostack));
                }
            }
            _ => {}
        }
    }
    gate::finish(3)
}
