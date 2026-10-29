//! Existing Conduit Face scene and typography, without environmental reconstruction.
use conduit_face::{Face, FaceBasis, FaceRole, FaceSubject, FaceText};
use conduitos::{
    display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget, typography},
    native_face_scene::NativeFaceScene,
};

/// Host-owned pre-birth status, never represented as the environmental Body Face.
pub fn unavailable(reason: &str) -> Face {
    Face::new(
        0,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "local/display-status".into(),
            role: FaceRole::Document,
            name: "Conduit".into(),
        }],
        vec![],
        vec![],
        vec![FaceText {
            subject: "local/display-status".into(),
            text: reason.into(),
        }],
    )
    .expect("bounded static host status")
}
struct Pixels<'a> {
    format: DisplayFormat,
    values: &'a mut [u32],
}
impl Pixels<'_> {
    fn index(&self, x: u32, y: u32) -> Result<usize, DisplayError> {
        if x >= self.format.width || y >= self.format.height {
            return Err(DisplayError::InvalidExtent);
        }
        Ok(y as usize * self.format.width as usize + x as usize)
    }
}
impl PixelTarget for Pixels<'_> {
    fn format(&self) -> DisplayFormat {
        self.format
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        let i = self.index(x, y)?;
        *self.values.get_mut(i).ok_or(DisplayError::BufferTooSmall)? = pixel;
        Ok(())
    }
}
impl RetainedPixelTarget for Pixels<'_> {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        self.values
            .get(self.index(x, y)?)
            .copied()
            .ok_or(DisplayError::BufferTooSmall)
    }
}

pub fn render(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    pixels: &mut [u32],
    preferred: Option<conduit_core::Unit>,
) -> Result<(usize, usize), String> {
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err("raster extent mismatch".into());
    }
    let (frame, page, pages, scale) = frame(face, width, height, page_tick, preferred)?;
    let mut target = Pixels {
        format: DisplayFormat {
            width: width.into(),
            height: height.into(),
            pitch: u32::from(width) * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        },
        values: pixels,
    };
    typography::render_scene_scaled(
        &mut target,
        &frame,
        |_, command| command.text_role().into(),
        scale,
    )
    .map_err(|e| format!("Face raster refused: {e:?}"))?;
    Ok((page, pages))
}

pub fn render_graphics(
    scene: &conduit_face::GraphicsScene,
    width: u16,
    height: u16,
    pixels: &mut [u32],
) -> Result<(), String> {
    if pixels.len() != usize::from(width) * usize::from(height) || height <= 36 || width == 0 {
        return Err("raster extent mismatch".into());
    }
    pixels.fill(0);
    let mut target = ChartViewport {
        width: usize::from(width),
        height: usize::from(height),
        pixels,
    };
    typography::render_scene_scaled(
        &mut target,
        scene,
        |_, command| command.text_role().into(),
        100,
    )
    .map(|_| ())
    .map_err(|e| format!("history raster: {e:?}"))
}
/// Scale the existing 640x360 logical chart into the admitted framebuffer,
/// reserving a separate Host status strip above it. No geometry is rewritten.
struct ChartViewport<'a> {
    width: usize,
    height: usize,
    pixels: &'a mut [u32],
}
impl PixelTarget for ChartViewport<'_> {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 640,
            height: 360,
            pitch: 640 * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if x >= 640 || y >= 360 {
            return Err(DisplayError::InvalidExtent);
        }
        let x = x as usize;
        let y = y as usize;
        let drawable = self.height - 36;
        for py in 36 + y * drawable / 360..36 + (y + 1) * drawable / 360 {
            for px in x * self.width / 640..(x + 1) * self.width / 640 {
                self.pixels[py * self.width + px] = pixel;
            }
        }
        Ok(())
    }
}
impl RetainedPixelTarget for ChartViewport<'_> {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        if x >= 640 || y >= 360 {
            return Err(DisplayError::InvalidExtent);
        }
        let px = x as usize * self.width / 640;
        let py = 36 + y as usize * (self.height - 36) / 360;
        self.pixels
            .get(py * self.width + px)
            .copied()
            .ok_or(DisplayError::BufferTooSmall)
    }
}

/// Host presentation status is separate from the unchanged producer Face.
pub fn render_notice(
    text: &str,
    width: u16,
    height: u16,
    pixels: &mut [u32],
) -> Result<(), String> {
    use conduit_face::{
        GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
    };
    let clip = LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                LayoutRect {
                    x: 0,
                    y: 0,
                    width,
                    height: 30,
                },
                clip,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .map_err(|e| format!("notice: {e:?}"))?,
        )
        .map_err(|e| format!("notice: {e:?}"))?;
    let bounded: String = text.chars().take(90).collect();
    scene
        .push(
            GraphicsCommand::text(
                LayoutRect {
                    x: 8,
                    y: 4,
                    width: width.saturating_sub(16),
                    height: 24,
                },
                clip,
                GraphicsPaintRole::Foreground,
                &bounded,
            )
            .map_err(|e| format!("notice: {e:?}"))?,
        )
        .map_err(|e| format!("notice: {e:?}"))?;
    let mut target = Pixels {
        format: DisplayFormat {
            width: width.into(),
            height: height.into(),
            pitch: u32::from(width) * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        },
        values: pixels,
    };
    typography::render_scene_scaled(
        &mut target,
        &scene,
        |_, command| command.text_role().into(),
        100,
    )
    .map(|_| ())
    .map_err(|e| format!("notice raster: {e:?}"))
}

fn frame(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    preferred: Option<conduit_core::Unit>,
) -> Result<(conduit_face::GraphicsScene, usize, usize, u16), String> {
    let property_face = face.clone();
    let property = crate::property_mask::frame(&property_face, width, height, 0, preferred);
    let mut scene = NativeFaceScene::prepare_with_input(face, width, height, false)
        .map_err(|e| format!("Face scene refused: {e:?}"))?;
    scene
        .set_local_notice("Read only; pages advance automatically")
        .map_err(|e| format!("Face notice refused: {e:?}"))?;
    let charts: Vec<_> = property_face
        .subjects
        .iter()
        .filter_map(|subject| {
            conduit_environment_history::HistoryGraph::prepare(&property_face, &subject.identity)
                .ok()
        })
        .collect();
    let native_pages = scene.pages();
    let property_pages = property.as_ref().map_or(0, |(_, _, pages)| *pages);
    let pages = charts.len() + native_pages + property_pages;
    let page = page_tick % pages;
    if page < charts.len() {
        let show = charts[page]
            .native_show_in_unit(preferred.unwrap_or(conduit_core::Unit::Celsius))
            .map_err(|e| format!("history graph refused: {e:?}"))?;
        return Ok((show.scene, page, pages, 100));
    }
    let content_page = page - charts.len();
    if content_page < property_pages {
        let (graphics, _, _) =
            crate::property_mask::frame(&property_face, width, height, content_page, preferred)
                .ok_or("property page refused")?;
        return Ok((graphics, page, pages, 100));
    }
    for _ in 0..content_page - property_pages {
        scene.turn_page(true);
    }
    let frame = scene
        .frame()
        .map_err(|e| format!("Face frame refused: {e:?}"))?;
    Ok((frame.scene, page, pages, scene.text_scale_percent()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensor_pages_do_not_hide_transcript_from_the_same_face() {
        let base = unavailable("Transcript: the greenhouse is quiet");
        let face = Face::new(
            9,
            base.basis,
            vec![
                base.subjects[0].clone(),
                FaceSubject {
                    identity: "sensor".into(),
                    role: FaceRole::Info,
                    name: "Greenhouse".into(),
                },
            ],
            vec![],
            vec![conduit_face::FaceProperty {
                subject: "sensor".into(),
                name: "reading".into(),
                value: conduit_face::FacePropertyValue::Text("Sensor marker".into()),
            }],
            base.text,
        )
        .unwrap();
        let original = face.clone();
        let (_, _, pages, _) = frame(face.clone(), 1024, 600, 0, None).unwrap();
        assert!(pages >= 2);
        let mut sensor = false;
        let mut transcript = false;
        for tick in 0..pages {
            let (scene, page, count, _) = frame(face.clone(), 1024, 600, tick, None).unwrap();
            assert_eq!((page, count), (tick, pages));
            sensor |= scene
                .commands()
                .iter()
                .any(|c| c.payload().contains("Sensor marker"));
            transcript |= scene
                .commands()
                .iter()
                .any(|c| c.payload().contains("greenhouse is quiet"));
        }
        assert!(
            sensor && transcript,
            "both contributions must remain perceptible"
        );
        assert_eq!(face, original);
    }
}
