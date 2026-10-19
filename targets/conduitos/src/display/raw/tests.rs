use super::*;
use alloc::{vec, vec::Vec};

fn format(pitch: u32) -> DisplayFormat {
    DisplayFormat {
        width: 17,
        height: 9,
        pitch,
        bits_per_pixel: 32,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
    }
}
fn render(residue: usize, word: bool, span: bool, pitch: u32) -> Vec<u8> {
    let format = format(pitch);
    let len = format.byte_len().unwrap();
    let mut bytes = vec![0xa5; len + 64];
    let offset = 16 + (4 - bytes.as_ptr().addr() % 4) % 4 + residue;
    // SAFETY: The live allocation contains the complete unique scanout extent.
    let address = unsafe { NonNull::new_unchecked(bytes.as_mut_ptr().add(offset)) };
    {
        // SAFETY: This owned memory supports byte and aligned word accesses;
        // neither the allocation nor its aliases are touched until display drops.
        let mut display = unsafe {
            if word {
                RawDisplay::new_word_writable(address, len, format)
            } else {
                RawDisplay::new(address, len, format)
            }
        }
        .unwrap();
        let mut row = [0; 13];
        let allocations = crate::test_allocations::allocations(|| {
            for y in 0..format.height {
                for (index, pixel) in row.iter_mut().enumerate() {
                    *pixel = (index as u32 * 0x01020304) ^ (y << 24) ^ 0xdeadbeef;
                }
                if span {
                    display.write_span(2, y, &row).unwrap();
                } else {
                    for (index, pixel) in row.iter().enumerate() {
                        display.write_pixel(2 + index as u32, y, *pixel).unwrap();
                    }
                }
            }
        });
        assert_eq!(allocations, 0);
    }
    assert!(bytes[..offset].iter().all(|byte| *byte == 0xa5));
    assert!(bytes[offset + len..].iter().all(|byte| *byte == 0xa5));
    bytes[offset..offset + len].to_vec()
}

#[test]
fn word_and_span_scanout_match_bytes_for_all_alignments_and_pitch_padding() {
    for pitch in [68, 69, 70, 71, 72, 80] {
        for residue in 0..4 {
            let expected = render(residue, false, false, pitch);
            for (word, span) in [(false, true), (true, false), (true, true)] {
                assert_eq!(
                    render(residue, word, span, pitch),
                    expected,
                    "pitch={pitch} residue={residue} word={word} span={span}"
                );
            }
            for row in expected.chunks_exact(pitch as usize) {
                assert!(row[..8].iter().all(|byte| *byte == 0xa5));
                assert!(row[60..].iter().all(|byte| *byte == 0xa5));
            }
        }
    }
}

#[test]
fn invalid_spans_and_lost_scanout_preserve_the_entire_buffer() {
    let format = format(72);
    let mut bytes = vec![0x5a; format.byte_len().unwrap()];
    {
        // SAFETY: This allocation remains unique and alive for the display scope.
        let mut display = unsafe {
            RawDisplay::new_word_writable(
                NonNull::new(bytes.as_mut_ptr()).unwrap(),
                bytes.len(),
                format,
            )
        }
        .unwrap();
        for (x, y, row) in [
            (17, 0, &[1][..]),
            (0, 9, &[1][..]),
            (16, 0, &[1, 2][..]),
            (u32::MAX, 0, &[1, 2][..]),
        ] {
            assert_eq!(
                display.write_span(x, y, row),
                Err(DisplayError::InvalidExtent)
            );
        }
        display.lose();
        assert_eq!(display.write_span(0, 0, &[1]), Err(DisplayError::Lost));
        assert_eq!(display.write_pixel(0, 0, 1), Err(DisplayError::Lost));
        assert_eq!(display.write_span(u32::MAX, u32::MAX, &[]), Ok(()));
    }
    assert!(bytes.iter().all(|byte| *byte == 0x5a));
}

#[test]
fn default_span_stops_at_the_exact_pixel_error_and_mutable_targets_forward_overrides() {
    struct FailAfter {
        writes: [u32; 4],
        count: usize,
    }
    impl PixelTarget for FailAfter {
        fn format(&self) -> DisplayFormat {
            format(72)
        }
        fn write_pixel(&mut self, x: u32, _: u32, pixel: u32) -> Result<(), DisplayError> {
            if self.count == 2 {
                return Err(DisplayError::Lost);
            }
            assert_eq!(x, 3 + self.count as u32);
            self.writes[self.count] = pixel;
            self.count += 1;
            Ok(())
        }
    }
    let mut target = FailAfter {
        writes: [0; 4],
        count: 0,
    };
    assert_eq!(
        target.write_span(3, 1, &[7, 8, 9, 10]),
        Err(DisplayError::Lost)
    );
    assert_eq!(target.writes, [7, 8, 0, 0]);
    struct SpanOnly(usize);
    impl PixelTarget for SpanOnly {
        fn format(&self) -> DisplayFormat {
            format(72)
        }
        fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
            panic!("lost span override")
        }
        fn write_span(&mut self, _: u32, _: u32, pixels: &[u32]) -> Result<(), DisplayError> {
            self.0 += pixels.len();
            Ok(())
        }
    }
    let mut target = SpanOnly(0);
    let mut borrowed = &mut target;
    PixelTarget::write_span(&mut borrowed, 0, 0, &[1, 2, 3]).unwrap();
    assert_eq!(target.0, 3);
}

#[test]
#[ignore = "manual optimized volatile scanout observation"]
fn volatile_scanout_performance() {
    use std::{hint::black_box, time::Instant};
    let format = DisplayFormat {
        width: 640,
        height: 480,
        pitch: 2560,
        ..format(72)
    };
    let row: Vec<_> = (0..640).map(|x| x * 0x010203).collect();
    for repeat in 0_u32..5 {
        for mode in if repeat.is_multiple_of(2) {
            [0, 1, 2]
        } else {
            [2, 1, 0]
        } {
            let mut bytes = vec![0; format.byte_len().unwrap()];
            let elapsed = {
                // SAFETY: The allocation is retained and uniquely used as scanout.
                let mut display = unsafe {
                    let address = NonNull::new(bytes.as_mut_ptr()).unwrap();
                    if mode == 0 {
                        RawDisplay::new(address, bytes.len(), format)
                    } else {
                        RawDisplay::new_word_writable(address, bytes.len(), format)
                    }
                }
                .unwrap();
                let start = Instant::now();
                for _ in 0..16 {
                    for y in 0..480 {
                        if mode == 2 {
                            display.write_span(0, y, black_box(&row)).unwrap();
                        } else {
                            for (x, pixel) in row.iter().enumerate() {
                                display.write_pixel(x as u32, y, black_box(*pixel)).unwrap();
                            }
                        }
                    }
                }
                start.elapsed().as_nanos()
            };
            black_box(&bytes);
            assert_eq!(&bytes[..4], &row[0].to_le_bytes());
            assert_eq!(&bytes[2556..2560], &row[639].to_le_bytes());
            println!("volatile-scanout mode={mode} repeat={repeat} frames=16 wall_ns={elapsed}");
        }
    }
}
