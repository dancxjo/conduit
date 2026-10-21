use super::*;
use alloc::vec;
// Compile the existing retained reference unchanged in this private test module.
use crate::native_compositor::{
    MAX_COMPOSITOR_PIXELS, MAX_COMPOSITOR_SURFACES, NativeCompositorError,
};
#[path = "../../native_compositor/surface_buffer.rs"]
mod reference_buffer;

#[test]
fn exact_admission_and_invalid_extents_refuse_before_effects() {
    for count in [0, 1, STAGING_PIXELS - 1, STAGING_PIXELS + 1] {
        let mut pixels = vec![0x123456; count];
        assert!(RasterStaging::admit(&mut pixels).is_err());
        assert!(pixels.iter().all(|pixel| *pixel == 0x123456));
    }
    let mut pixels = vec![0x123456; STAGING_PIXELS];
    let mut staging = RasterStaging::admit(&mut pixels).unwrap();
    assert!(staging.pixels.iter().all(|pixel| *pixel == 0));
    // Test-only sentinels distinguish refusal from accidental zeroing effects.
    staging.pixels.fill(0xabcdef);
    assert!(staging.lease(0, 1).is_err());
    assert!(staging.lease(1, 0).is_err());
    assert!(staging.lease(2049, 1024).is_err());
    assert!(staging.lease(u16::MAX, u16::MAX).is_err());
    assert!(staging.pixels.iter().all(|pixel| *pixel == 0xabcdef));
}

#[test]
fn row_bounds_and_atomic_spans_preserve_prior_pixels() {
    let mut pixels = vec![0; STAGING_PIXELS];
    let mut staging = RasterStaging::admit(&mut pixels).unwrap();
    let mut lease = staging.lease(3, 2).unwrap();
    assert_eq!(
        lease.format(),
        DisplayFormat {
            width: 3,
            height: 2,
            pitch: 12,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0
        }
    );
    lease.write_span(0, 1, &[1, 2, 3]).unwrap();
    assert_eq!(
        lease.write_span(2, 1, &[7, 8]),
        Err(DisplayError::InvalidExtent)
    );
    assert_eq!(
        lease.write_span(u32::MAX, 0, &[7, 8]),
        Err(DisplayError::InvalidExtent)
    );
    assert_eq!(
        lease.write_span(0, 2, &[7]),
        Err(DisplayError::InvalidExtent)
    );
    assert_eq!(lease.read_pixel(3, 0), Err(DisplayError::InvalidExtent));
    assert_eq!(lease.write_pixel(3, 0, 9), Err(DisplayError::InvalidExtent));
    assert_eq!(
        lease
            .upload_chunks()
            .flatten()
            .copied()
            .collect::<alloc::vec::Vec<_>>(),
        [0, 0, 0, 1, 2, 3]
    );
    assert_eq!(lease.write_span(u32::MAX, u32::MAX, &[]), Ok(()));
}

#[test]
fn lease_cleanup_runs_after_error_and_successive_extents_start_clear() {
    let mut pixels = vec![0; STAGING_PIXELS];
    {
        let mut staging = RasterStaging::admit(&mut pixels).unwrap();
        let result = (|| {
            let mut lease = staging.lease(8, 8)?;
            lease.write_pixel(7, 7, 0xabcdef)?;
            lease.write_pixel(8, 7, 1)
        })();
        assert_eq!(result, Err(DisplayError::InvalidExtent));
        for (width, height) in [(2, 3), (16, 16), (3, 2)] {
            let mut lease = staging.lease(width, height).unwrap();
            assert!(lease.upload_chunks().flatten().all(|pixel| *pixel == 0));
            lease.write_pixel(0, 0, 42).unwrap();
        }
    }
    assert!(pixels.iter().all(|pixel| *pixel == 0));
}

#[test]
fn staging_and_upload_chunks_allocate_nothing_after_admission() {
    let mut pixels = vec![0; STAGING_PIXELS];
    let mut staging = RasterStaging::admit(&mut pixels).unwrap();
    let allocations = crate::test_allocations::allocations(|| {
        for (width, height) in [(1024, 2048), (127, 17), (4, 3)] {
            let mut lease = staging.lease(width, height).unwrap();
            lease.write_span(0, 0, &[0x102030; 4]).unwrap();
            assert_eq!(lease.read_pixel(0, 0), Ok(0x102030));
            assert!(
                lease
                    .upload_chunks()
                    .all(|chunk| !chunk.is_empty() && chunk.len() <= UPLOAD_CHUNK_PIXELS)
            );
            assert_eq!(
                lease
                    .upload_chunks()
                    .map(|chunk| chunk.len())
                    .sum::<usize>(),
                lease.pixel_count()
            );
        }
    });
    assert_eq!(allocations, 0);
}

#[cfg(feature = "native-compositor")]
#[test]
fn actual_typography_blending_matches_existing_surface_buffer_without_allocation() {
    use crate::display::typography::{TextRole, render_text};
    use conduit_face::LayoutRect;
    let mut pixels = vec![0; STAGING_PIXELS];
    let mut staging = RasterStaging::admit(&mut pixels).unwrap();
    let mut lease = staging.lease(32, 32).unwrap();
    let mut reference = reference_buffer::SurfaceBuffer::new(32, 32).unwrap();
    for y in 0..32 {
        for x in 0..32 {
            lease.write_pixel(x, y, 0xa0204060).unwrap();
            reference.write_pixel(x, y, 0xa0204060).unwrap();
        }
    }
    let bounds = LayoutRect {
        x: 2,
        y: 0,
        width: 28,
        height: 32,
    };
    let clip = LayoutRect {
        x: 5,
        y: 5,
        width: 12,
        height: 12,
    };
    let allocations = crate::test_allocations::allocations(|| {
        let actual =
            render_text(&mut lease, "W", TextRole::Heading, bounds, clip, 0xffffff).unwrap();
        let expected = render_text(
            &mut reference,
            "W",
            TextRole::Heading,
            bounds,
            clip,
            0xffffff,
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert!(actual.pixels_written > 0);
    });
    assert_eq!(allocations, 0);
    let mut smooth = false;
    for y in 0..32 {
        for x in 0..32 {
            let actual = lease.read_pixel(x, y).unwrap();
            assert_eq!(actual, reference.read_pixel(x, y).unwrap());
            assert_eq!(actual >> 24, 0xa0);
            if !(5..17).contains(&x) || !(5..17).contains(&y) {
                assert_eq!(actual, 0xa0204060);
            }
            if actual != 0xa0204060 && actual != 0xa0ffffff {
                smooth = true;
            }
        }
    }
    assert!(smooth);
}
