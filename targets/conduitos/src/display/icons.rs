//! Original fixed 24px native drawings for the existing semantic icon keys.
//! Geometry is rasterized at compile time, not parsed or cached during frames.

use super::{DisplayError, DisplayReceipt, PixelTarget, clipped, put};
use conduit_presentation::{GraphicsCommand, GraphicsSymbol, PresentationIconKey};
mod symbols;

const EDGE: usize = 24;
struct Icon {
    key: PresentationIconKey,
    rows: [u32; EDGE],
}

const fn line(x: i32, y: i32, ax: i32, ay: i32, bx: i32, by: i32) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let px = x - ax;
    let py = y - ay;
    let length = dx * dx + dy * dy;
    let dot = px * dx + py * dy;
    let cross = px * dy - py * dx;
    dot >= 0 && dot <= length && cross * cross <= length
}

const fn ring(x: i32, y: i32, cx: i32, cy: i32, radius: i32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    let distance = dx * dx + dy * dy;
    distance >= (radius - 1) * (radius - 1) && distance <= (radius + 1) * (radius + 1)
}

const fn frame(x: i32, y: i32, left: i32, top: i32, right: i32, bottom: i32) -> bool {
    line(x, y, left, top, right, top)
        || line(x, y, right, top, right, bottom)
        || line(x, y, right, bottom, left, bottom)
        || line(x, y, left, bottom, left, top)
}

const fn letter_a(x: i32, y: i32) -> bool {
    line(x, y, 5, 20, 12, 3) || line(x, y, 12, 3, 19, 20) || line(x, y, 8, 14, 16, 14)
}

const fn ink(key: PresentationIconKey, x: i32, y: i32) -> bool {
    use PresentationIconKey::*;
    match key {
        Clock => ring(x, y, 12, 12, 9) || line(x, y, 12, 6, 12, 12) || line(x, y, 12, 12, 17, 15),
        Repeat2 => {
            line(x, y, 5, 6, 19, 6)
                || line(x, y, 19, 6, 19, 11)
                || line(x, y, 19, 6, 16, 3)
                || line(x, y, 5, 18, 19, 18)
                || line(x, y, 5, 13, 5, 18)
                || line(x, y, 5, 18, 8, 21)
        }
        Presentation => {
            frame(x, y, 3, 3, 21, 16) || line(x, y, 12, 16, 12, 21) || line(x, y, 7, 21, 17, 21)
        }
        Type => line(x, y, 4, 4, 20, 4) || line(x, y, 12, 4, 12, 20) || line(x, y, 8, 20, 16, 20),
        CaseUpper => letter_a(x, y),
        Combine => {
            line(x, y, 3, 5, 8, 5)
                || line(x, y, 8, 5, 13, 12)
                || line(x, y, 3, 19, 8, 19)
                || line(x, y, 8, 19, 13, 12)
                || line(x, y, 13, 12, 21, 12)
                || line(x, y, 17, 8, 21, 12)
                || line(x, y, 17, 16, 21, 12)
        }
        Tally5 => {
            line(x, y, 4, 4, 4, 20)
                || line(x, y, 9, 4, 9, 20)
                || line(x, y, 14, 4, 14, 20)
                || line(x, y, 19, 4, 19, 20)
                || line(x, y, 2, 17, 21, 7)
        }
        ChartColumnsIncreasing => {
            frame(x, y, 3, 15, 7, 21) || frame(x, y, 10, 10, 14, 21) || frame(x, y, 17, 3, 21, 21)
        }
        FileOutput => {
            line(x, y, 4, 3, 14, 3)
                || line(x, y, 4, 3, 4, 21)
                || line(x, y, 4, 21, 14, 21)
                || line(x, y, 14, 3, 18, 7)
                || line(x, y, 18, 7, 18, 9)
                || line(x, y, 10, 14, 22, 14)
                || line(x, y, 18, 10, 22, 14)
                || line(x, y, 18, 18, 22, 14)
        }
        Keyboard => {
            frame(x, y, 2, 5, 22, 19)
                || line(x, y, 7, 16, 17, 16)
                || (y >= 8 && y <= 12 && y % 4 == 0 && x >= 5 && x <= 19 && x % 4 == 1)
        }
        GenericGear => {
            ring(x, y, 12, 12, 6)
                || ring(x, y, 12, 12, 2)
                || line(x, y, 12, 2, 12, 5)
                || line(x, y, 12, 19, 12, 22)
                || line(x, y, 2, 12, 5, 12)
                || line(x, y, 19, 12, 22, 12)
                || line(x, y, 5, 5, 7, 7)
                || line(x, y, 17, 17, 19, 19)
                || line(x, y, 5, 19, 7, 17)
                || line(x, y, 17, 7, 19, 5)
        }
    }
}

const fn prepare(key: PresentationIconKey) -> Icon {
    let mut rows = [0; EDGE];
    let mut y = 0;
    while y < EDGE {
        let mut x = 0;
        while x < EDGE {
            if ink(key, x as i32, y as i32) {
                rows[y] |= 1 << x;
            }
            x += 1;
        }
        y += 1;
    }
    Icon { key, rows }
}

const ICONS: [Icon; 11] = [
    prepare(PresentationIconKey::Clock),
    prepare(PresentationIconKey::Repeat2),
    prepare(PresentationIconKey::Presentation),
    prepare(PresentationIconKey::Type),
    prepare(PresentationIconKey::CaseUpper),
    prepare(PresentationIconKey::Combine),
    prepare(PresentationIconKey::Tally5),
    prepare(PresentationIconKey::ChartColumnsIncreasing),
    prepare(PresentationIconKey::FileOutput),
    prepare(PresentationIconKey::Keyboard),
    prepare(PresentationIconKey::GenericGear),
];

pub(super) fn render(
    target: &mut impl PixelTarget,
    command: &GraphicsCommand,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let rows = if let Some(symbol) = GraphicsSymbol::from_token(command.payload()) {
        symbols::raster(symbol)
    } else {
        let key = PresentationIconKey::from_token(command.payload())
            .ok_or(DisplayError::InvalidExtent)?;
        &ICONS
            .iter()
            .find(|icon| icon.key == key)
            .ok_or(DisplayError::InvalidExtent)?
            .rows
    };
    let Some(clip) = clipped(command.bounds, command.clip, target.format()) else {
        return Ok(());
    };
    for (row, bits) in rows.iter().enumerate() {
        for column in 0..EDGE {
            if bits & (1 << column) == 0 {
                continue;
            }
            let x = i32::from(command.bounds.x) + column as i32;
            let y = i32::from(command.bounds.y) + row as i32;
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Target {
        pixels: [u32; 32 * 32],
    }
    impl PixelTarget for Target {
        fn format(&self) -> super::super::DisplayFormat {
            super::super::DisplayFormat {
                width: 32,
                height: 32,
                pitch: 128,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
            self.pixels[y as usize * 32 + x as usize] = pixel;
            Ok(())
        }
    }

    #[test]
    fn icon_leaf_uses_fixed_raster_and_respects_its_exact_clip() {
        let mut target = Target {
            pixels: [0; 32 * 32],
        };
        let bounds = conduit_presentation::LayoutRect {
            x: 2,
            y: 2,
            width: 24,
            height: 24,
        };
        let clip = conduit_presentation::LayoutRect {
            x: 5,
            y: 5,
            width: 8,
            height: 8,
        };
        let command = GraphicsCommand::icon(
            bounds,
            clip,
            conduit_presentation::GraphicsPaintRole::Foreground,
            PresentationIconKey::Clock,
        )
        .unwrap();
        let mut receipt = DisplayReceipt::default();
        render(&mut target, &command, 1, &mut receipt).unwrap();
        assert!(receipt.pixels_written > 0 && receipt.pixels_written <= 64);
        for y in 0..32 {
            for x in 0..32 {
                if target.pixels[y * 32 + x] != 0 {
                    assert!((5..13).contains(&x) && (5..13).contains(&y));
                }
            }
        }
    }

    #[test]
    fn every_semantic_key_has_one_distinct_nonempty_bounded_drawing() {
        assert!(core::mem::size_of_val(&ICONS) < 2048);
        for key in PresentationIconKey::ALL {
            let icons: alloc::vec::Vec<_> = ICONS.iter().filter(|icon| icon.key == key).collect();
            assert_eq!(icons.len(), 1);
            assert!(icons[0].rows.iter().any(|row| *row != 0));
            assert!(icons[0].rows.iter().all(|row| row >> EDGE == 0));
        }
        for (index, icon) in ICONS.iter().enumerate() {
            assert!(
                ICONS[..index]
                    .iter()
                    .all(|previous| previous.rows != icon.rows)
            );
        }
    }
}
