#![no_std]
#![no_main]
//! Link-only Cortex-M0+ measurement, not a bootable board image.
use conduit_speech::{
    EnglishPhoneme as P, EnglishPosition as W, EnglishStress as S, RealizationInput, Renderer,
    VoiceEvent,
};
static EVENT: VoiceEvent = VoiceEvent::segment(RealizationInput {
    phoneme: P::aa,
    position: W::initial,
    stress: S::primary,
});
static mut OUTPUT: [i16; 128] = [0; 128];
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        let event = unsafe { core::ptr::read_volatile(&EVENT) };
        let events = [event];
        if let Ok(mut renderer) = Renderer::prepare(&events) {
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
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
