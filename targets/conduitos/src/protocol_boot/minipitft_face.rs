//! Diagnostic CPU-rendered Face frame, before physical display acceptance.
//!
//! This uses the ordinary native Face scene and typography renderer. Rendering
//! into RAM is not a Mask execution or a Show acknowledgement. The caller must
//! supply its actual producing basis; this helper invents no Body or execution.
use crate::{
    display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget, typography},
    native_face_scene::{FaceSceneError, NativeFaceScene},
};
use alloc::{vec, vec::Vec};
use conduit_face::{
    Face, FaceBasis, FaceError, FaceRole, FaceSubject, FaceText, GraphicsCommand, GraphicsError,
    GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};

pub(crate) const WIDTH: u16 = 240;
pub(crate) const HEIGHT: u16 = 240;
const MARKER: u16 = 12;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticFrameRefusal {
    Face(FaceError),
    Scene(FaceSceneError),
    Graphics(GraphicsError),
    Display(DisplayError),
    Paginated,
}

fn prepare_scene(
    basis: FaceBasis,
    revision: u64,
) -> Result<NativeFaceScene, DiagnosticFrameRefusal> {
    let face = Face::new(
        revision,
        basis,
        vec![FaceSubject {
            identity: "display-diagnostic".into(),
            role: FaceRole::Document,
            name: "ConduitOS".into(),
        }],
        vec![],
        vec![],
        vec![FaceText {
            subject: "display-diagnostic".into(),
            text: "Mini PiTFT display test".into(),
        }],
    )
    .map_err(DiagnosticFrameRefusal::Face)?;
    // Input is deliberately unavailable in this first display diagnostic.
    // No controls, interaction mapping or acknowledged Show are constructed.
    let mut scene = NativeFaceScene::prepare_with_input(face, WIDTH, HEIGHT, false)
        .map_err(DiagnosticFrameRefusal::Scene)?;
    scene
        .set_local_notice("Buttons pending")
        .map_err(DiagnosticFrameRefusal::Scene)?;
    if scene.pages() != 1 {
        return Err(DiagnosticFrameRefusal::Paginated);
    }
    Ok(scene)
}

fn markers() -> Result<GraphicsScene, DiagnosticFrameRefusal> {
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width: WIDTH,
        height: HEIGHT,
    };
    let mut scene = GraphicsScene::empty();
    for (x, y, paint) in [
        (0, 0, GraphicsPaintRole::Accent),
        (WIDTH - MARKER, 0, GraphicsPaintRole::Warning),
        (0, HEIGHT - MARKER, GraphicsPaintRole::Muted),
        (
            WIDTH - MARKER,
            HEIGHT - MARKER,
            GraphicsPaintRole::Foreground,
        ),
    ] {
        scene
            .push(
                GraphicsCommand::rect(
                    LayoutRect {
                        x: x as i16,
                        y: y as i16,
                        width: MARKER,
                        height: MARKER,
                    },
                    screen,
                    paint,
                    GraphicsShapeStyle::Fill,
                )
                .map_err(DiagnosticFrameRefusal::Graphics)?,
            )
            .map_err(DiagnosticFrameRefusal::Graphics)?;
    }
    Ok(scene)
}

// Only a checked borrowed RAM target; all drawing and coverage blending stay
// in the existing renderer. No memory or scanout authority is represented here.
struct DiagnosticPixels<'a> {
    format: DisplayFormat,
    pixels: &'a mut [u32],
}
impl DiagnosticPixels<'_> {
    fn index(&self, x: u32, y: u32) -> Result<usize, DisplayError> {
        if x >= self.format.width || y >= self.format.height {
            return Err(DisplayError::InvalidExtent);
        }
        usize::try_from(u64::from(y) * u64::from(self.format.width) + u64::from(x))
            .map_err(|_| DisplayError::InvalidExtent)
    }
}
impl PixelTarget for DiagnosticPixels<'_> {
    fn format(&self) -> DisplayFormat {
        self.format
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        let index = self.index(x, y)?;
        *self
            .pixels
            .get_mut(index)
            .ok_or(DisplayError::BufferTooSmall)? = pixel;
        Ok(())
    }
}
impl RetainedPixelTarget for DiagnosticPixels<'_> {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        self.pixels
            .get(self.index(x, y)?)
            .copied()
            .ok_or(DisplayError::BufferTooSmall)
    }
}

/// Freeze a complete 240×240 `0x00RRGGBB` diagnostic raster for packed encoding.
/// No rendering receipt returned here asserts that a physical display showed it.
pub(crate) fn render(basis: FaceBasis, revision: u64) -> Result<Vec<u32>, DiagnosticFrameRefusal> {
    let prepared = prepare_scene(basis, revision)?;
    let frame = prepared.frame().map_err(DiagnosticFrameRefusal::Scene)?;
    let markers = markers()?;
    let format = DisplayFormat {
        width: u32::from(WIDTH),
        height: u32::from(HEIGHT),
        pitch: u32::from(WIDTH) * 4,
        bits_per_pixel: 32,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
    };
    let mut pixels = vec![0; usize::from(WIDTH) * usize::from(HEIGHT)];
    {
        let mut target = DiagnosticPixels {
            format,
            pixels: &mut pixels,
        };
        typography::render_scene_scaled(
            &mut target,
            &frame.scene,
            |_, command| command.text_role().into(),
            prepared.text_scale_percent(),
        )
        .map_err(DiagnosticFrameRefusal::Display)?;
        // Four conventional corner colors expose cropping and both axes of
        // software orientation. Existing rectangle rasterization draws them.
        typography::render_scene_configured(
            &mut target,
            &markers,
            |_, command| command.text_role().into(),
            100,
            |role| match role {
                GraphicsPaintRole::Accent => 0x00ff0000,
                GraphicsPaintRole::Warning => 0x0000ff00,
                GraphicsPaintRole::Muted => 0x000000ff,
                _ => 0x00ffffff,
            },
        )
        .map_err(DiagnosticFrameRefusal::Display)?;
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::GraphicsCommandKind;
    fn basis() -> FaceBasis {
        let producer = crate::native_face_scene::tests::fixture::producer_plan();
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(producer.source_document_id),
            checked_plot_id: Some(producer.checked_plot_id),
            expanded_plot_id: Some(producer.expanded_plot_id),
            plan_id: Some(producer.plan_id),
            active_play_id: None,
            sign_ids: vec![],
        }
    }
    #[test]
    fn minipitft_diagnostic_face_preserves_real_basis_and_fits_one_240_pixel_page() {
        let basis = basis();
        let prepared = prepare_scene(basis.clone(), 7).unwrap();
        assert_eq!(prepared.face().basis, basis);
        assert_eq!(prepared.face().revision, 7);
        assert!(prepared.face().actions.is_empty());
        let frame = prepared.frame().unwrap();
        assert_eq!(frame.pages, 1);
        assert!(frame.hits.is_empty());
        for text in ["ConduitOS", "Mini PiTFT display test", "Buttons pending"] {
            assert!(
                frame
                    .scene
                    .commands()
                    .iter()
                    .any(|command| command.payload() == text)
            );
        }
        for command in frame.scene.commands() {
            assert!(command.bounds.x >= 0 && command.bounds.y >= 0);
            assert!(u32::from(command.bounds.width) + command.bounds.x as u32 <= u32::from(WIDTH));
            assert!(
                u32::from(command.bounds.height) + command.bounds.y as u32 <= u32::from(HEIGHT)
            );
            if command.kind == GraphicsCommandKind::Text {
                let text = typography::TextLayout::new_scaled(
                    command.payload(),
                    command.text_role().into(),
                    command.bounds.width,
                    100,
                )
                .unwrap();
                assert!(text.finish_height() <= u32::from(command.bounds.height));
            }
        }
    }
    #[test]
    fn minipitft_diagnostic_cpu_frame_has_exact_extent_and_distinct_complete_corners() {
        let pixels = render(basis(), 1).unwrap();
        assert_eq!(pixels.len(), 240 * 240);
        for (x, y, color) in [
            (0, 0, 0xff0000),
            (228, 0, 0x00ff00),
            (0, 228, 0x0000ff),
            (228, 228, 0xffffff),
        ] {
            for dy in 0..12 {
                for dx in 0..12 {
                    assert_eq!(pixels[(y + dy) * 240 + x + dx], color);
                }
            }
        }
        // Text and chrome actually changed the interior beyond a flat fill.
        let interior: alloc::collections::BTreeSet<_> =
            pixels[12 * 240..228 * 240].iter().collect();
        assert!(interior.len() >= 4);
    }
}
