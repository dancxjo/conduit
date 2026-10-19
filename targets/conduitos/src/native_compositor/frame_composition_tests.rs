//! Differential framebuffer, acknowledgement and partial-failure proof.
use super::super::*;
use super::{compose_damage, reference};
use crate::display::{DisplayFormat, PixelTarget};

#[allow(dead_code)]
mod fixture {
    use crate as conduitos;
    include!("../../tests/common/native_compositor_fixture.rs");
}
#[allow(dead_code)]
mod display {
    use crate as conduitos;
    use alloc::vec::Vec;
    include!("../../tests/common/native_compositor.rs");
}

fn surfaces(seed: u32) -> Vec<CompositorSurface> {
    let (face, plan, main, _, mut compositor) = fixture::fixture(1);
    fixture::update(&mut compositor, &face, &plan, &main, fixture::MAIN);
    let template = &compositor.surfaces[0];
    let mut random = seed;
    let mut next = || {
        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
        random
    };
    (0..MAX_COMPOSITOR_SURFACES)
        .map(|index| {
            let bounds = LayoutRect {
                x: (next() % 48) as i16 - 12,
                y: (next() % 24) as i16 - 6,
                width: (next() % 24 + 1) as u16,
                height: (next() % 16 + 1) as u16,
            };
            let mut buffer = SurfaceBuffer::new(bounds.width, bounds.height).unwrap();
            for (offset, pixel) in buffer.pixels.iter_mut().enumerate() {
                *pixel = (index as u32 + 1) * 1024 + offset as u32;
            }
            CompositorSurface {
                surface_id: format!("surface/{index}"),
                bounds,
                z: (next() % 3) as u8,
                visible: !next().is_multiple_of(4),
                buffer,
                visual_expression: None,
                binding: template.binding.clone(),
                receipt: if next().is_multiple_of(5) {
                    None
                } else {
                    template.receipt.clone()
                },
                input_ready: false,
            }
        })
        .collect()
}

#[test]
fn spans_match_reference_pixels_coverage_and_partial_failure() {
    for seed in 0..64 {
        let surfaces = surfaces(seed);
        for damage in [
            vec![DamageRect {
                x: 0,
                y: 0,
                width: 32,
                height: 16,
            }],
            vec![DamageRect {
                x: 8,
                y: 5,
                width: 1,
                height: 1,
            }],
            vec![
                DamageRect {
                    x: 3,
                    y: 2,
                    width: 13,
                    height: 7,
                },
                DamageRect {
                    x: 9,
                    y: 4,
                    width: 15,
                    height: 9,
                },
            ],
        ] {
            for fail_after in [None, Some(0), Some(13), Some(127)] {
                let mut actual = display::MemoryDisplay::new();
                actual.fail_after = fail_after;
                let mut expected = actual.clone();
                let format = actual.format();
                let focus = (seed.is_multiple_of(2)).then_some("surface/3");
                let cursor = (!seed.is_multiple_of(3)).then_some((seed % 33, seed % 17));
                let mut result = None;
                let allocations = crate::test_allocations::allocations(|| {
                    result = Some(compose_damage(
                        &mut actual,
                        format,
                        &surfaces,
                        &damage,
                        focus,
                        cursor,
                        seed.is_multiple_of(2),
                    ));
                });
                assert_eq!(allocations, 0);
                let result = result.unwrap();
                let reference = reference::compose_damage(
                    &mut expected,
                    format,
                    &surfaces,
                    &damage,
                    focus,
                    cursor,
                    seed.is_multiple_of(2),
                );
                assert_eq!(result, reference, "seed={seed} fail={fail_after:?}");
                assert_eq!(
                    actual.pixels, expected.pixels,
                    "seed={seed} fail={fail_after:?}"
                );
                assert_eq!(actual.writes, expected.writes);
            }
        }
    }
}

#[test]
fn invalid_damage_refuses_without_scanout() {
    let surfaces = surfaces(7);
    let mut target = display::MemoryDisplay::new();
    let format = target.format();
    for rect in [
        DamageRect {
            x: 31,
            y: 0,
            width: 2,
            height: 1,
        },
        DamageRect {
            x: u32::MAX,
            y: 0,
            width: 2,
            height: 1,
        },
    ] {
        assert_eq!(
            compose_damage(&mut target, format, &surfaces, &[rect], None, None, false),
            Err(NativeCompositorError::InvalidBounds)
        );
        assert_eq!(target.writes, 0);
    }
}

#[test]
#[ignore = "manual optimized compositor performance observation"]
fn span_compositor_performance() {
    use std::{hint::black_box, time::Instant};
    let mut surfaces = surfaces(42);
    for surface in &mut surfaces {
        surface.bounds.x *= 12;
        surface.bounds.y *= 16;
        surface.bounds.width *= 16;
        surface.bounds.height *= 16;
        surface.buffer = SurfaceBuffer::new(surface.bounds.width, surface.bounds.height).unwrap();
        for (index, pixel) in surface.buffer.pixels.iter_mut().enumerate() {
            *pixel = index as u32;
        }
    }
    let mut target = display::MemoryDisplay::new();
    target.format = DisplayFormat {
        width: 640,
        height: 480,
        pitch: 2560,
        ..target.format
    };
    target.pixels.resize(640 * 480, 0);
    let format = target.format;
    for (name, damage) in [
        (
            "full",
            DamageRect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
        ),
        (
            "tiny",
            DamageRect {
                x: 8,
                y: 5,
                width: 1,
                height: 1,
            },
        ),
        (
            "scroll",
            DamageRect {
                x: 0,
                y: 0,
                width: 320,
                height: 240,
            },
        ),
        (
            "hover",
            DamageRect {
                x: 5,
                y: 2,
                width: 16,
                height: 16,
            },
        ),
        (
            "typing",
            DamageRect {
                x: 0,
                y: 0,
                width: 200,
                height: 16,
            },
        ),
    ] {
        let frames = match name {
            "tiny" => 65536,
            "hover" => 4096,
            "typing" => 1024,
            _ => 16,
        };
        for repeat in 0_u32..5 {
            let mut samples = [0; 2];
            for implementation in if repeat.is_multiple_of(2) {
                [0, 1]
            } else {
                [1, 0]
            } {
                let start = Instant::now();
                for _ in 0..frames {
                    let compose = if implementation == 0 {
                        reference::compose_damage
                    } else {
                        compose_damage
                    };
                    black_box(
                        compose(
                            black_box(&mut target),
                            format,
                            &surfaces,
                            &[damage],
                            Some("surface/3"),
                            Some((8, 5)),
                            true,
                        )
                        .unwrap(),
                    );
                }
                samples[implementation] = start.elapsed().as_nanos();
            }
            println!(
                "span-compositor case={name} repeat={repeat} frames={frames} reference_ns={} spans_ns={}",
                samples[0], samples[1]
            );
        }
    }
}

#[test]
fn focus_and_cursor_edges_keep_surface_interiors_on_the_span_path() {
    struct Spans {
        inner: display::MemoryDisplay,
        bulk_pixels: usize,
    }
    impl PixelTarget for Spans {
        fn format(&self) -> DisplayFormat {
            self.inner.format()
        }
        fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
            self.inner.write_pixel(x, y, pixel)
        }
        fn write_span(&mut self, x: u32, y: u32, pixels: &[u32]) -> Result<(), DisplayError> {
            self.bulk_pixels += pixels.len();
            self.inner.write_span(x, y, pixels)
        }
    }
    let (face, plan, main, _, mut compositor) = fixture::fixture(1);
    compositor
        .place_surface(
            fixture::MAIN,
            LayoutRect {
                x: 0,
                y: 0,
                width: 32,
                height: 16,
            },
            0,
        )
        .unwrap();
    fixture::update(&mut compositor, &face, &plan, &main, fixture::MAIN);
    let mut target = Spans {
        inner: display::MemoryDisplay::new(),
        bulk_pixels: 0,
    };
    let mut expected = target.inner.clone();
    let format = target.format();
    let damage = [DamageRect {
        x: 0,
        y: 0,
        width: 32,
        height: 16,
    }];
    let result = compose_damage(
        &mut target,
        format,
        &compositor.surfaces,
        &damage,
        Some(fixture::MAIN),
        Some((15, 7)),
        false,
    )
    .unwrap();
    let reference = reference::compose_damage(
        &mut expected,
        format,
        &compositor.surfaces,
        &damage,
        Some(fixture::MAIN),
        Some((15, 7)),
        false,
    )
    .unwrap();
    assert_eq!(result, reference);
    assert_eq!(target.inner.pixels, expected.pixels);
    assert!(
        target.bulk_pixels > 32 * 16 / 2,
        "overlay edges must not make a full focused surface fall back to per-pixel writes"
    );
}
