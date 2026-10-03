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
static mut CHECKSUM: u32 = 0;
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        let event = unsafe { core::ptr::read_volatile(&EVENT) };
        let events = [event];
        if let Ok(mut renderer) = Renderer::prepare(&events) {
            while !renderer.is_complete() {
                // The link-only probe has one caller and no interrupt handlers.
                // Render directly into caller-owned static storage; volatile
                // reads retain all produced PCM without a stack bounce buffer.
                let output_ptr = &raw mut OUTPUT;
                let output = unsafe { &mut *output_ptr };
                if let Ok(count) = renderer.render(output) {
                    let mut checksum = 0u32;
                    for index in 0..count {
                        let sample = unsafe {
                            core::ptr::read_volatile((&raw const OUTPUT).cast::<i16>().add(index))
                        };
                        checksum = checksum.wrapping_add(sample as u16 as u32);
                    }
                    unsafe {
                        core::ptr::write_volatile(&raw mut CHECKSUM, checksum);
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
