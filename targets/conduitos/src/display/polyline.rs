//! Finite integer straight-segment raster; clips pixels without inventing samples.
use super::*;
pub(super) fn render(
    target: &mut impl PixelTarget,
    format: DisplayFormat,
    command: &GraphicsCommand,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let line = command
        .polyline_geometry()
        .ok_or(DisplayError::InvalidExtent)?;
    let Some(clip) = clipped(command.bounds, command.clip, format) else {
        return Ok(());
    };
    for pair in line.points().windows(2) {
        let (mut x, mut y) = (i32::from(*pair[0].x()), i32::from(*pair[0].y()));
        let (end_x, end_y) = (i32::from(*pair[1].x()), i32::from(*pair[1].y()));
        let dx = (end_x - x).abs();
        let dy = -(end_y - y).abs();
        let sx = if x < end_x { 1 } else { -1 };
        let sy = if y < end_y { 1 } else { -1 };
        let mut error = dx + dy;
        // Exact coordinate bounds and <=16 points bound all segment work.
        loop {
            if x >= i32::from(clip.x)
                && y >= i32::from(clip.y)
                && x < i32::from(clip.x) + i32::from(clip.width)
                && y < i32::from(clip.y) + i32::from(clip.height)
            {
                put(target, x as u32, y as u32, color, receipt)?;
            }
            if x == end_x && y == end_y {
                break;
            }
            let twice = error * 2;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Pixels([u32; 64]);
    impl PixelTarget for Pixels {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 8,
                height: 8,
                pitch: 32,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
            self.0[(y * 8 + x) as usize] = pixel;
            Ok(())
        }
    }
    #[test]
    fn diagonal_is_a_line_and_clip_never_writes_outside_selection() {
        let line = conduit_face::GraphicsPolyline::new(&[
            conduit_face::GraphicsPoint::new(0, 0).unwrap(),
            conduit_face::GraphicsPoint::new(7, 7).unwrap(),
        ])
        .unwrap();
        let command = GraphicsCommand::polyline(
            line,
            LayoutRect {
                x: 2,
                y: 2,
                width: 3,
                height: 3,
            },
            GraphicsPaintRole::Foreground,
        )
        .unwrap();
        let mut target = Pixels([0; 64]);
        let format = target.format();
        let mut receipt = DisplayReceipt::default();
        render(&mut target, format, &command, 99, &mut receipt).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    target.0[y * 8 + x],
                    if x == y && (2..5).contains(&x) { 99 } else { 0 }
                );
            }
        }
        assert_eq!(receipt.pixels_written, 3);
    }
}
