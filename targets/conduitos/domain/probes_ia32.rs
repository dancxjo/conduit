//! IA-32 hostile implementation instructions, absent from product images.
pub use crate::probe_gate::mutate_gate;
use crate::{frame::TextFrame, gate};

pub unsafe fn run(frame: &TextFrame) -> ! {
    unsafe {
        match frame.probe {
            1 => {
                core::arch::asm!("mov eax, [{0}]", in(reg) frame.target as u32, out("eax") _, options(nostack))
            }
            2 => {
                core::arch::asm!("mov byte ptr [{0}], 0xff", in(reg) frame.target as u32, options(nostack))
            }
            3 => core::arch::asm!("call {0}", in(reg) frame.target as u32),
            4 => core::arch::asm!("out dx, al", in("dx") 0x80u16, in("al") 0u8, options(nostack)),
            5 => core::arch::asm!("cli", options(nostack)),
            6 => loop {
                core::arch::asm!("pause", options(nomem, nostack));
            },
            // SSE is part of this implementation ABI: disturb it to test restoration.
            7 => {
                core::arch::asm!(
                    "pcmpeqd xmm0, xmm0",
                    "push 0x7f80",
                    "ldmxcsr [esp]",
                    "add esp, 4",
                    "push 0xf7f",
                    "fldcw [esp]",
                    "add esp, 4"
                );
                gate::finish(0);
            }
            8 => core::arch::asm!("syscall", options(nostack)),
            9 => core::arch::asm!("sysenter", options(nostack)),
            10 => {
                core::arch::asm!("xor ecx, ecx", "xor edx, edx", "div ecx", out("eax") _, out("ecx") _, out("edx") _, options(nostack))
            }
            11 => core::arch::asm!("int3", options(nostack)),
            12 => core::arch::asm!("pushfd", "or dword ptr [esp], 0x100", "popfd", "nop"),
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
