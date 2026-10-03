#![no_std]
#![no_main]
//! Text-plus-renderer link/section probe; not bootable firmware or stack proof.
use conduit_speech::{pronounce, Renderer, VoiceBoundary, VoiceEvent};
static TEXT: [u8; 49] = *b"Hello world! This is a native speech synthesizer.";
static mut EVENTS: [VoiceEvent; 64] = [VoiceEvent::boundary(VoiceBoundary::phrase); 64];
static mut OUTPUT: [i16; 128] = [0; 128];
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        let bytes = unsafe { core::ptr::read_volatile(&TEXT) };
        if let Ok(text) = core::str::from_utf8(&bytes) {
            let storage = unsafe { &mut *(&raw mut EVENTS) };
            if let Ok(batch) = pronounce(text, storage) {
                if let Ok(mut renderer) = Renderer::prepare(batch.events()) {
                    let mut block = [0; 128];
                    while !renderer.is_complete() {
                        let _ = renderer.render(&mut block);
                        unsafe {
                            core::ptr::write_volatile(&raw mut OUTPUT, block);
                        }
                    }
                }
            }
        }
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
