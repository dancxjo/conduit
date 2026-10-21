//! Deliberately hostile proof artifact. Never linked into the product renderer.
#[path = "../domain/memory.rs"]
mod compiler_memory;
use conduitos_compositor_core::wire::{self, Request, Response};
const EXCHANGE: usize = 0x0060_1000;
/// # Safety
/// Root installs the fixed service grants and supplies only that exchange address.
/// This proof artifact intentionally faults, loops, or violates reply correlation.
pub unsafe fn run(exchange: *mut u8, behavior: u8) -> ! {
    if exchange as usize != EXCHANGE {
        finish(3);
    }
    let bytes = unsafe { core::slice::from_raw_parts_mut(exchange, 4096) };
    let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let Ok((mut envelope, request)) = wire::decode_request(bytes.get(..length).unwrap_or(&[]))
    else {
        finish(3);
    };
    let response = match request {
        Request::Initialize { width, height }
            if width > 0 && height > 0 && envelope.request == 1 =>
        {
            Response::Unit
        }
        Request::DamagePending => match behavior {
            0 => {
                envelope.request += 1;
                Response::DamagePending(false)
            }
            1 => Response::Keyboard(None),
            2 => unsafe {
                core::arch::asm!("cli", options(nomem, nostack));
                finish(3)
            },
            3 => loop {
                core::hint::spin_loop();
            },
            _ => finish(3),
        },
        _ => finish(3),
    };
    if wire::encode_response(envelope, response, bytes).is_err() {
        finish(3);
    }
    finish(0)
}
fn finish(status: u32) -> ! {
    unsafe {
        core::arch::asm!("int 0x80",in("eax")status,options(noreturn));
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    finish(3)
}
