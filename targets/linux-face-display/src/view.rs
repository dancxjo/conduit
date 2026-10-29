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
) -> Result<(usize, usize), String> {
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err("raster extent mismatch".into());
    }
    let mut scene = NativeFaceScene::prepare_with_input(face, width, height, false)
        .map_err(|e| format!("Face scene refused: {e:?}"))?;
    scene
        .set_local_notice("Read only; pages advance automatically")
        .map_err(|e| format!("Face notice refused: {e:?}"))?;
    let pages = scene.pages();
    let page = page_tick % pages;
    for _ in 0..page {
        scene.turn_page(true);
    }
    let frame = scene
        .frame()
        .map_err(|e| format!("Face frame refused: {e:?}"))?;
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
        &frame.scene,
        |_, command| command.text_role().into(),
        scene.text_scale_percent(),
    )
    .map_err(|e| format!("Face raster refused: {e:?}"))?;
    Ok((page, pages))
}
