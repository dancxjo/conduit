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
        let (x0, y0) = (i32::from(*pair[0].x()), i32::from(*pair[0].y()));
        let (x1, y1) = (i32::from(*pair[1].x()), i32::from(*pair[1].y()));
        let (dx, dy) = ((x1 - x0).abs(), (y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let horizontal = dx >= dy;
        let steps = if horizontal {
            visible_steps(
                x0,
                sx,
                dx,
                i32::from(clip.x),
                i32::from(clip.x) + i32::from(clip.width),
            )
        } else {
            visible_steps(
                y0,
                sy,
                dy,
                i32::from(clip.y),
                i32::from(clip.y) + i32::from(clip.height),
            )
        };
        let Some(steps) = steps else {
            continue;
        };
        // Evaluate the original segment's Bresenham phase directly. Clipping
        // never restarts the error at a rounded intersection. Only visible
        // major-axis coordinates are visited: <=clip width/height per segment.
        for step in steps {
            let rounded = |minor: i32, major: i32| {
                ((i64::from(minor) * i64::from(step) + i64::from(major / 2)) / i64::from(major))
                    as i32
            };
            let (x, y) = if horizontal {
                (x0 + sx * step, y0 + sy * rounded(dy, dx))
            } else {
                (x0 + sx * rounded(dx, dy), y0 + sy * step)
            };
            if x >= i32::from(clip.x)
                && y >= i32::from(clip.y)
                && x < i32::from(clip.x) + i32::from(clip.width)
                && y < i32::from(clip.y) + i32::from(clip.height)
            {
                put(target, x as u32, y as u32, color, receipt)?;
            }
        }
    }
    Ok(())
}

fn visible_steps(
    start: i32,
    direction: i32,
    length: i32,
    low: i32,
    high: i32,
) -> Option<core::ops::RangeInclusive<i32>> {
    let (first, last) = if direction > 0 {
        (low - start, high - 1 - start)
    } else {
        (start - (high - 1), start - low)
    };
    let first = first.max(0);
    let last = last.min(length);
    (first <= last).then_some(first..=last)
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
    #[test]
    fn clipped_raster_matches_full_original_segment_phase_in_all_octants() {
        let clip = LayoutRect {
            x: 2,
            y: 2,
            width: 3,
            height: 3,
        };
        for (start, end) in [
            ((-4, -1), (12, 6)),
            ((12, 6), (-4, -1)),
            ((-1, -4), (6, 12)),
            ((6, 12), (-1, -4)),
            ((0, 7), (7, 0)),
            ((7, 0), (0, 7)),
            ((3, -9), (3, 12)),
            ((-9, 3), (12, 3)),
        ] {
            let line = conduit_face::GraphicsPolyline::new(&[
                conduit_face::GraphicsPoint::new(start.0, start.1).unwrap(),
                conduit_face::GraphicsPoint::new(end.0, end.1).unwrap(),
            ])
            .unwrap();
            let command =
                GraphicsCommand::polyline(line, clip, GraphicsPaintRole::Foreground).unwrap();
            let mut target = Pixels([0; 64]);
            let format = target.format();
            render(
                &mut target,
                format,
                &command,
                99,
                &mut DisplayReceipt::default(),
            )
            .unwrap();
            let mut reference = [0; 64];
            let (mut x, mut y) = (i32::from(start.0), i32::from(start.1));
            let (end_x, end_y) = (i32::from(end.0), i32::from(end.1));
            let dx = (end_x - x).abs();
            let dy = -(end_y - y).abs();
            let sx = if x < end_x { 1 } else { -1 };
            let sy = if y < end_y { 1 } else { -1 };
            let mut error = dx + dy;
            loop {
                if (2..5).contains(&x) && (2..5).contains(&y) {
                    reference[(y * 8 + x) as usize] = 99;
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
            assert_eq!(target.0, reference, "{start:?}->{end:?}");
        }
        assert_eq!(visible_steps(-10000, 1, 20000, 2, 5).unwrap().count(), 3);
    }
}
