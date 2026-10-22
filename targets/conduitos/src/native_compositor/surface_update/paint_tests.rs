//! Actual rounded raster output under admitted palette and forced-color policy.
use super::*;
use crate::display::{DisplayFormat, PixelTarget, RetainedPixelTarget};
use conduit_face::{GraphicsCommand, GraphicsPaintRole as Paint, GraphicsShapeStyle as Shape};

struct Surface([u32; 32 * 24]);
impl PixelTarget for Surface {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 32,
            height: 24,
            pitch: 128,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        self.0[y as usize * 32 + x as usize] = pixel;
        Ok(())
    }
}
impl RetainedPixelTarget for Surface {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        Ok(self.0[y as usize * 32 + x as usize])
    }
}
fn rect(x: i16, y: i16, width: u16, height: u16) -> LayoutRect {
    LayoutRect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn surface_border_and_text_paints_survive_real_raster_and_forced_colors() {
    use conduit_face::{
        GraphicsCommand, VisualExpression, VisualPalette, VisualUserConstraints,
        admit_visual_expression,
    };
    for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
        for forced_colors in [false, true] {
            let mut requested = VisualExpression::phosphor();
            requested.palette = palette;
            let admitted = admit_visual_expression(
                Ok(requested),
                VisualUserConstraints {
                    forced_colors,
                    reduced_motion: true,
                    minimum_text_scale_percent: 100,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            let mut target = Surface([0xDEADBE; 32 * 24]);
            let format = target.format();
            let extent = rect(0, 0, 32, 24);
            let inset = rect(2, 2, 12, 16);
            let mut scene = GraphicsScene::empty();
            for (bounds, paint, shape) in [
                (extent, Paint::Background, Shape::Fill),
                (inset, Paint::Surface, Shape::RoundedFill),
                (inset, Paint::Border, Shape::RoundedStroke),
                (rect(22, 2, 8, 8), Paint::Foreground, Shape::Fill),
            ] {
                scene
                    .push(GraphicsCommand::rect(bounds, extent, paint, shape).unwrap())
                    .unwrap();
            }
            let receipt = crate::display::typography::render_scene_configured(
                &mut target,
                &scene,
                |_, c| c.text_role().into(),
                100,
                |paint| visual_paint(format, paint, Some(&admitted)),
            )
            .unwrap();
            let background = visual_paint(format, Paint::Background, Some(&admitted));
            let surface = visual_paint(format, Paint::Surface, Some(&admitted));
            let border = visual_paint(format, Paint::Border, Some(&admitted));
            let text = visual_paint(format, Paint::Foreground, Some(&admitted));
            assert_eq!(target.read_pixel(0, 0).unwrap(), background);
            assert_eq!(target.read_pixel(8, 8).unwrap(), surface);
            assert_eq!(target.read_pixel(2, 9).unwrap(), border);
            assert_eq!(target.read_pixel(23, 3).unwrap(), text);
            assert_ne!(surface, border);
            assert_ne!(surface, text);
            assert!(contrast(surface, text) >= 4.5);
            assert!(contrast(surface, border) >= 3.0);
            for role in [
                Paint::Muted,
                Paint::Accent,
                Paint::Warning,
                Paint::Success,
                Paint::Danger,
            ] {
                assert!(contrast(surface, visual_paint(format, role, Some(&admitted))) >= 4.5);
            }
            // Primary labels use Background ink over an Accent fill, including
            // the platform's black/white forced-color mapping.
            assert!(
                contrast(
                    background,
                    visual_paint(format, Paint::Accent, Some(&admitted))
                ) >= 4.5
            );
            if forced_colors {
                assert_eq!(surface, background);
                assert_eq!(border, text);
            } else {
                assert_ne!(surface, background);
                assert_ne!(border, text);
            }
            assert_eq!(receipt.commands, 4);
            assert!(receipt.pixels_written <= 4 * 32 * 24);
        }
    }
}

#[test]
fn smooth_geometry_blends_background_and_clipped_redraw_matches_full() {
    use crate::display::typography::{TextRole, render_scene};
    use conduit_face::{FaceIconKey, GraphicsScene, LayoutRect};
    for (width, height) in [(12u16, 10u16), (24, 20), (28, 22)] {
        let bounds = LayoutRect {
            x: 2,
            y: 2,
            width,
            height,
        };
        let full = LayoutRect {
            x: 0,
            y: 0,
            width: 32,
            height: 24,
        };
        let clip = LayoutRect {
            x: 9,
            y: 5,
            width: 12,
            height: 13,
        };
        for command in [
            GraphicsCommand::rect(bounds, full, Paint::Foreground, Shape::RoundedFill).unwrap(),
            GraphicsCommand::rect(bounds, full, Paint::Foreground, Shape::RoundedStroke).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Clock).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Repeat2).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Face).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Type).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::CaseUpper).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Combine).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Tally5).unwrap(),
            GraphicsCommand::icon(
                bounds,
                full,
                Paint::Foreground,
                FaceIconKey::ChartColumnsIncreasing,
            )
            .unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::FileOutput)
                .unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Keyboard).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::GenericGear)
                .unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Body).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Wake).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Plan).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Play).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Host).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Port).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Line).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Status).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Warning).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Close).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Back).unwrap(),
            GraphicsCommand::icon(bounds, full, Paint::Foreground, FaceIconKey::Confirm).unwrap(),
        ] {
            let background = 0xa0204060;
            let mut surface = Surface([background; 32 * 24]);
            let mut scene = GraphicsScene::empty();
            scene.push(command).unwrap();
            let receipt = render_scene(&mut surface, &scene, |_, _| TextRole::Body).unwrap();
            assert!(
                receipt.pixels_written > 0
                    && receipt.pixels_written <= u32::from(width) * u32::from(height)
            );
            let foreground = surface.format().pixel(
                crate::display::FOREGROUND.0,
                crate::display::FOREGROUND.1,
                crate::display::FOREGROUND.2,
            ) | 0xa0000000;
            assert!(
                surface
                    .0
                    .iter()
                    .any(|pixel| *pixel != background && *pixel != foreground)
            );
            assert!(surface.0.iter().all(|pixel| *pixel >> 24 == 0xa0));
            let expected = surface.0;
            let mut clipped = command;
            clipped.clip = clip;
            let mut scene = GraphicsScene::empty();
            scene.push(clipped).unwrap();
            surface.0.fill(background);
            render_scene(&mut surface, &scene, |_, _| TextRole::Body).unwrap();
            for y in 0..24 {
                for x in 0..32 {
                    let index = y * 32 + x;
                    assert_eq!(
                        surface.0[index],
                        if (9..21).contains(&x) && (5..18).contains(&y) {
                            expected[index]
                        } else {
                            background
                        }
                    );
                }
            }
        }
    }
}

// Test-only sRGB luminance; production palette resolution stays integer-only.
fn contrast(a: u32, b: u32) -> f64 {
    let luminance = |pixel: u32| {
        [16, 8, 0]
            .into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(shift, weight)| {
                let value = f64::from((pixel >> shift) & 255) / 255.0;
                weight
                    * if value <= 0.04045 {
                        value / 12.92
                    } else {
                        ((value + 0.055) / 1.055).powf(2.4)
                    }
            })
            .sum::<f64>()
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
