//! Full native-size canvas and stride coverage through the shared product Face.
use super::*;
use crate::display::{DisplayFormat, RawDisplay};
use core::ptr::NonNull;

#[test]
fn native_panel_face_writes_the_entire_canvas_without_touching_stride_padding() {
    const WIDTH: usize = 1024;
    const HEIGHT: usize = 600;
    const SENTINEL: u32 = 0xdead_beef;
    for padding in [0, 16] {
        let stride = WIDTH + padding;
        let mut pixels = alloc::vec![SENTINEL; stride * HEIGHT];
        // This test retains the backing allocation until the sole display owner
        // is dropped; no read or alias accesses it while scanout writes occur.
        let display = unsafe {
            RawDisplay::new_word_writable(
                NonNull::new(pixels.as_mut_ptr().cast()).unwrap(),
                pixels.len() * 4,
                DisplayFormat {
                    width: WIDTH as u32,
                    height: HEIGHT as u32,
                    pitch: (stride * 4) as u32,
                    bits_per_pixel: 32,
                    red_shift: 0,
                    green_shift: 8,
                    blue_shift: 16,
                },
            )
            .unwrap()
        };
        let ids = identities();
        let make = make();
        let frame =
            FirmwareFace::prepare(display, &ids, &offer(&ids, &make), &make, [7; 32]).unwrap();
        drop(frame);
        for row in pixels.chunks_exact(stride) {
            assert!(row[..WIDTH].iter().all(|pixel| *pixel != SENTINEL));
            assert!(row[WIDTH..].iter().all(|pixel| *pixel == SENTINEL));
        }
        let background = pixels[WIDTH - 1];
        let right_ink = pixels
            .chunks_exact(stride)
            .map(|row| {
                row[2 * WIDTH / 3..WIDTH]
                    .iter()
                    .filter(|pixel| **pixel != background)
                    .count()
            })
            .sum::<usize>();
        std::println!(
            "native 1024x600, stride {stride}: right-third non-background pixels={right_ink}"
        );
        // Optional internal diagnostic artifact; not a product configuration.
        if padding == 0
            && let Some(path) = std::env::var_os("CONDUITOS_TEST_FRAME_SNAPSHOT")
        {
            let mut bytes = alloc::format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
            for pixel in &pixels {
                bytes.extend_from_slice(&pixel.to_le_bytes()[..3]);
            }
            std::fs::write(path, bytes).unwrap();
        }
    }
}
