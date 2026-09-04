//! Clipped low-level drawing helpers for the native GUI composition.

use crate::{
    canvas::softbuffer_to_rgb888,
    font::BitmapFont,
    icon::{draw_icon, Icon},
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Primitive, Size},
    primitives::{Line, PrimitiveStyle, Rectangle},
    Drawable,
};
use patchbay_model::{ApplicationTheme, ThemeColor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub(super) struct RegionMetrics {
    pub header_height: i32,
    pub footer_height: i32,
    pub nav_width: i32,
    pub inspector_width: i32,
}

impl PixelRect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && y >= f64::from(self.y)
            && x < f64::from(self.x) + f64::from(self.width)
            && y < f64::from(self.y) + f64::from(self.height)
    }

    fn rectangle(self) -> Rectangle {
        Rectangle::new(
            Point::new(self.x, self.y),
            Size::new(self.width, self.height),
        )
    }
}

pub(super) fn draw_regions<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    width: i32,
    height: i32,
    metrics: RegionMetrics,
    theme: &ApplicationTheme,
) {
    let RegionMetrics {
        header_height,
        footer_height,
        nav_width,
        inspector_width,
    } = metrics;
    fill_rect(
        target,
        PixelRect {
            x: 0,
            y: 0,
            width: positive(width),
            height: positive(height),
        },
        theme.background,
    );
    fill_rect(
        target,
        PixelRect {
            x: 0,
            y: 0,
            width: positive(width),
            height: positive(header_height),
        },
        theme.surface,
    );
    fill_rect(
        target,
        PixelRect {
            x: 0,
            y: header_height,
            width: positive(nav_width),
            height: positive(height - header_height - footer_height),
        },
        theme.surface,
    );
    fill_rect(
        target,
        PixelRect {
            x: width - inspector_width,
            y: header_height,
            width: positive(inspector_width),
            height: positive(height - header_height - footer_height),
        },
        theme.surface,
    );
    frame_rect(
        target,
        PixelRect {
            x: 0,
            y: 0,
            width: positive(width),
            height: positive(height),
        },
        theme.structure_primary,
        1,
    );
    line(
        target,
        Point::new(0, header_height),
        Point::new(width, header_height),
        theme.structure_primary,
    );
    line(
        target,
        Point::new(nav_width, header_height),
        Point::new(nav_width, height - footer_height),
        theme.structure_secondary,
    );
    line(
        target,
        Point::new(width - inspector_width, header_height),
        Point::new(width - inspector_width, height - footer_height),
        theme.structure_secondary,
    );
    line(
        target,
        Point::new(0, height - footer_height),
        Point::new(width, height - footer_height),
        theme.structure_primary,
    );
}

pub(super) fn icon_label<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    icon: Icon,
    origin: Point,
    label: &str,
    color: ThemeColor,
) {
    draw_icon(target, icon, origin, rgb(color));
    text(target, origin + Point::new(22, 0), label, color);
}

pub(super) fn text<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    origin: Point,
    value: &str,
    color: ThemeColor,
) {
    let _ = BitmapFont::draw_text(target, origin, value, rgb(color));
}

pub(super) fn fill_rect<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bounds: PixelRect,
    color: ThemeColor,
) {
    let _ = bounds
        .rectangle()
        .into_styled(PrimitiveStyle::with_fill(rgb(color)))
        .draw(target);
}

pub(super) fn frame_rect<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bounds: PixelRect,
    color: ThemeColor,
    width: u32,
) {
    let _ = bounds
        .rectangle()
        .into_styled(PrimitiveStyle::with_stroke(rgb(color), width))
        .draw(target);
}

pub(super) fn line<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    start: Point,
    end: Point,
    color: ThemeColor,
) {
    let _ = Line::new(start, end)
        .into_styled(PrimitiveStyle::with_stroke(rgb(color), 1))
        .draw(target);
}

pub(super) fn rgb(color: ThemeColor) -> Rgb888 {
    softbuffer_to_rgb888(color.packed_rgb())
}

pub(super) fn positive(value: i32) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}
