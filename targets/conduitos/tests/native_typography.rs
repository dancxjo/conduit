#![cfg(feature = "native-compositor")]

use conduit_presentation::LayoutRect;
use conduitos::display::typography::{TextRole, render_text};
use conduitos::display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct Counted;
#[global_allocator]
static ALLOCATOR: Counted = Counted;

fn record() {
    let _ = TRACK.try_with(|track| {
        if track.get() {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
    });
}

// SAFETY: all storage operations delegate unchanged layouts and pointers to
// System. Thread-local counters observe only this proof thread, not the harness.
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

struct Surface {
    pixels: [u32; 256 * 128],
}
impl PixelTarget for Surface {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 256,
            height: 128,
            pitch: 1024,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        self.pixels[y as usize * 256 + x as usize] = pixel;
        Ok(())
    }
}
impl RetainedPixelTarget for Surface {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        Ok(self.pixels[y as usize * 256 + x as usize])
    }
}

#[test]
fn first_and_repeated_frames_have_no_runtime_font_allocations() {
    let mut surface = Surface {
        pixels: [0; 256 * 128],
    };
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 256,
        height: 128,
    };
    ALLOCATIONS.with(|count| count.set(0));
    TRACK.with(|track| track.set(true));
    let mut written = 0_u64;
    for _ in 0..128 {
        for role in [
            TextRole::Body,
            TextRole::Code,
            TextRole::Heading,
            TextRole::Title,
            TextRole::Label,
        ] {
            surface.pixels.fill(0x102030);
            let receipt = render_text(
                &mut surface,
                "Trần Tấn Tiến\nFádípẹ̀ HELLO\nunsupported: \u{10ffff}",
                role,
                bounds,
                bounds,
                0xffffff,
            )
            .unwrap();
            written += u64::from(receipt.pixels_written);
        }
    }
    TRACK.with(|track| track.set(false));
    assert!(written > 0);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
