#![no_std]
#![no_main]
//! Text-plus-renderer link/section probe; not bootable firmware or stack proof.
use conduit_speech::{pronounce, Renderer, VoiceBoundary, VoiceEvent};
static TEXT: [u8; 49] = *b"Hello world! This is a native speech synthesizer.";
static mut EVENTS: [VoiceEvent; 64] = [VoiceEvent::boundary(VoiceBoundary::phrase); 64];
static mut OUTPUT: [i16; 128] = [0; 128];
static mut CHECKSUM: u32 = 0;
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        let bytes = unsafe { core::ptr::read_volatile(&TEXT) };
        if let Ok(text) = core::str::from_utf8(&bytes) {
            let storage_ptr = &raw mut EVENTS;
            let storage = unsafe { &mut *storage_ptr };
            if let Ok(batch) = pronounce(text, storage) {
                if let Ok(mut renderer) = Renderer::prepare(batch.events()) {
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
                                    core::ptr::read_volatile(
                                        (&raw const OUTPUT).cast::<i16>().add(index),
                                    )
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
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
