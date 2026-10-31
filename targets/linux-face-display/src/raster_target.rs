//! Native pixel targets and the shared Look-token paint resolver.
use crate::view::STATUS_STRIP_HEIGHT;
use conduitos::display::{
    DisplayError, DisplayFormat, DisplayReceipt, PixelTarget, RetainedPixelTarget, typography,
};
pub(crate) struct Pixels<'a> {
    pub format: DisplayFormat,
    pub values: &'a mut [u32],
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
pub(crate) struct ChartViewport<'a> {
    pub width: usize,
    pub height: usize,
    pub pixels: &'a mut [u32],
}
impl PixelTarget for ChartViewport<'_> {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: self.width as u32,
            height: (self.height - STATUS_STRIP_HEIGHT) as u32,
            pitch: self.width as u32 * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if x as usize >= self.width || y as usize >= self.height - STATUS_STRIP_HEIGHT {
            return Err(DisplayError::InvalidExtent);
        }
        self.pixels[(y as usize + STATUS_STRIP_HEIGHT) * self.width + x as usize] = pixel;
        Ok(())
    }
}
impl RetainedPixelTarget for ChartViewport<'_> {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        if x as usize >= self.width || y as usize >= self.height - STATUS_STRIP_HEIGHT {
            return Err(DisplayError::InvalidExtent);
        }
        let px = x as usize;
        let py = STATUS_STRIP_HEIGHT + y as usize;
        self.pixels
            .get(py * self.width + px)
            .copied()
            .ok_or(DisplayError::BufferTooSmall)
    }
}

pub fn paint_scene(
    target: &mut impl RetainedPixelTarget,
    scene: &conduit_face::GraphicsScene,
    scale: u16,
    look: Option<&conduit_face::LookInfo>,
) -> Result<DisplayReceipt, DisplayError> {
    let format = target.format();
    typography::render_scene_selected(
        target,
        scene,
        |_, command| command.text_role().into(),
        scale,
        selected_typography(look).map_err(|_| DisplayError::InvalidExtent)?,
        |role| conduitos::display::visual_look_paint(format, role, None, look),
    )
}
pub fn panel(
    scene: &conduit_face::GraphicsScene,
    width: u16,
    height: u16,
    pixels: &mut [u32],
    look: &conduit_face::LookInfo,
) -> Result<(), String> {
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err("panel pixel extent".into());
    }
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
    paint_scene(&mut target, scene, 100, Some(look))
        .map(|_| ())
        .map_err(|e| format!("panel raster: {e:?}"))
}

/// Measurement and paint select the same fixed, made font resource.
/// No runtime font loader or Face mutation is involved.
pub fn selected_typography(
    look: Option<&conduit_face::LookInfo>,
) -> Result<typography::Typography, typography::LookTypographyRefusal> {
    use conduit_face::{LookVisualFacetOption, VisualTypography, VisualTypographyPreference};
    let Some(LookVisualFacetOption::Some(visual)) = look.map(|value| value.visual()) else {
        return Ok(typography::Typography::EDITORIAL);
    };
    let intent = match visual.expression().typography() {
        VisualTypographyPreference::Engineering => VisualTypography::Engineering,
        _ => VisualTypography::Editorial,
    };
    typography::Typography::with_look_metrics(intent, visual.tokens(), 200)
}
