//! Allocation-free initialized pixels for scanout before a product Face exists.
use super::{DisplayError, PixelTarget};

pub const BOOT_CANVAS_RGB: [u8; 3] = [0xcc, 0xcc, 0xff];

/// Fill every visible pixel using the admitted channel layout and row stride.
/// Commit completes before Root enables peripherals that may light the panel.
pub fn prefill(target: &mut impl PixelTarget) -> Result<(), DisplayError> {
    let format = target.format().validate()?;
    let [red, green, blue] = BOOT_CANVAS_RGB;
    let row = [format.pixel(red, green, blue); 256];
    for y in 0..format.height {
        let mut x = 0;
        while x < format.width {
            let count = (format.width - x).min(row.len() as u32);
            target.write_span(x, y, &row[..count as usize])?;
            x += count;
        }
    }
    target.commit_frame()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{DisplayFormat, RawDisplay};
    use alloc::vec;
    use core::ptr::NonNull;

    #[test]
    fn periwinkle_covers_last_column_and_row_without_touching_padding() {
        for (red_shift, blue_shift) in [(16, 0), (0, 16)] {
            let format = DisplayFormat {
                width: 257,
                height: 3,
                pitch: 260 * 4,
                bits_per_pixel: 32,
                red_shift,
                green_shift: 8,
                blue_shift,
            };
            let length = format.byte_len().unwrap();
            let mut bytes = vec![0x5a; length + 8];
            {
                // SAFETY: this test exclusively owns the complete pixel range.
                let mut display = unsafe {
                    RawDisplay::new(
                        NonNull::new(bytes.as_mut_ptr().add(4)).unwrap(),
                        length,
                        format,
                    )
                }
                .unwrap();
                prefill(&mut display).unwrap();
            }
            assert_eq!(&bytes[..4], &[0x5a; 4]);
            assert_eq!(&bytes[length + 4..], &[0x5a; 4]);
            let pixel = format.pixel(0xcc, 0xcc, 0xff).to_le_bytes();
            for y in 0..format.height as usize {
                let row =
                    &bytes[4 + y * format.pitch as usize..4 + (y + 1) * format.pitch as usize];
                for value in row[..format.width as usize * 4].as_chunks::<4>().0 {
                    assert_eq!(*value, pixel);
                }
                assert!(row[format.width as usize * 4..].iter().all(|&b| b == 0x5a));
            }
        }
    }

    struct Refusing {
        committed: bool,
    }
    impl PixelTarget for Refusing {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 1,
                height: 1,
                pitch: 4,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
            Err(DisplayError::Lost)
        }
        fn commit_frame(&mut self) -> Result<(), DisplayError> {
            self.committed = true;
            Ok(())
        }
    }
    #[test]
    fn failed_prefill_is_never_committed() {
        let mut display = Refusing { committed: false };
        assert_eq!(prefill(&mut display), Err(DisplayError::Lost));
        assert!(!display.committed);
    }
}
