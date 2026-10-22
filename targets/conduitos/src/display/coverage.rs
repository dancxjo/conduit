//! Fixed 4×4 coverage samples per visible pixel, blended once against retained RGB.
use super::*;

pub(super) fn render_command(
    target: &mut impl RetainedPixelTarget,
    format: DisplayFormat,
    command: &GraphicsCommand,
    receipt: &mut DisplayReceipt,
    color: u32,
) -> Result<(), DisplayError> {
    let Some(clip) = clipped(command.bounds, command.clip, format) else {
        return Ok(());
    };
    match command.kind {
        GraphicsCommandKind::Rect
            if matches!(
                command.style,
                GraphicsShapeStyle::RoundedFill | GraphicsShapeStyle::RoundedStroke
            ) =>
        {
            rounded::render_smooth(target, command.bounds, clip, command.style, color, receipt)
        }
        GraphicsCommandKind::Icon => icons::draw_smooth(target, command, clip, color, receipt),
        _ => render_command_with_color(target, format, command, receipt, color),
    }
}

pub(super) fn raster(
    target: &mut impl RetainedPixelTarget,
    bounds: LayoutRect,
    clip: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
    inside: impl Fn(i32, i32) -> bool,
) -> Result<(), DisplayError> {
    let format = target.format().validate()?;
    for y in i32::from(clip.y)..i32::from(clip.y) + i32::from(clip.height) {
        for x in i32::from(clip.x)..i32::from(clip.x) + i32::from(clip.width) {
            let mut samples = 0u32;
            for sy in [1, 3, 5, 7] {
                for sx in [1, 3, 5, 7] {
                    samples += u32::from(inside(
                        (x - i32::from(bounds.x)) * 8 + sx,
                        (y - i32::from(bounds.y)) * 8 + sy,
                    ));
                }
            }
            if samples == 0 {
                continue;
            }
            let background = target.read_pixel(x as u32, y as u32)?;
            let mut pixel = background;
            for shift in [format.red_shift, format.green_shift, format.blue_shift] {
                let fg = (color >> shift) & 255;
                let bg = (background >> shift) & 255;
                let channel = (fg * samples + bg * (16 - samples) + 8) / 16;
                pixel = (pixel & !(255 << shift)) | (channel << shift);
            }
            put(target, x as u32, y as u32, pixel, receipt)?;
        }
    }
    Ok(())
}
