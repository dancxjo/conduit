use conduit_home_model::HomeView;
use conduit_home_native::NativeHomePresentation;
use conduit_presentation::ApplicationNodeState;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::Text,
};

pub struct Canvas<'a> {
    pixels: &'a mut [u32],
    width: usize,
    height: usize,
}

impl<'a> Canvas<'a> {
    pub fn new(pixels: &'a mut [u32], width: usize, height: usize) -> Self {
        Self {
            pixels,
            width,
            height,
        }
    }
}

pub fn render(canvas: &mut Canvas<'_>, view: &NativeHomePresentation, notice: &str) {
    canvas.clear(Rgb888::new(8, 18, 26)).ok();
    let accent = Rgb888::new(84, 238, 196);
    let pale = Rgb888::new(210, 230, 239);
    let muted = Rgb888::new(120, 157, 171);
    let panel = Rgb888::new(13, 31, 39);
    let selected = Rgb888::new(18, 65, 55);
    let heading = MonoTextStyle::new(&FONT_6X10, pale);
    let body = MonoTextStyle::new(&FONT_6X10, pale);
    let quiet = MonoTextStyle::new(&FONT_6X10, muted);
    let bright = MonoTextStyle::new(&FONT_6X10, accent);

    Text::new("△  CONDUIT", Point::new(28, 34), bright)
        .draw(canvas)
        .ok();
    Text::new(&view.title.to_uppercase(), Point::new(28, 68), heading)
        .draw(canvas)
        .ok();
    Text::new(
        "SYSTEMS FOR A BRIGHTER TOMORROW.",
        Point::new(28, 88),
        quiet,
    )
    .draw(canvas)
    .ok();

    if view.view == HomeView::Launcher {
        render_launcher(canvas, view, body, bright, panel, selected, muted);
    } else {
        render_linear(canvas, view, body, quiet, panel, selected);
    }

    let status_y = canvas.height.saturating_sub(44) as i32;
    Rectangle::new(
        Point::new(0, status_y - 18),
        Size::new(canvas.width as u32, 62),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb888::new(6, 14, 21)))
    .draw(canvas)
    .ok();
    Text::new(&truncate(notice, 150), Point::new(28, status_y + 10), quiet)
        .draw(canvas)
        .ok();
}

fn render_launcher(
    canvas: &mut Canvas<'_>,
    view: &NativeHomePresentation,
    body: MonoTextStyle<'_, Rgb888>,
    bright: MonoTextStyle<'_, Rgb888>,
    panel: Rgb888,
    selected_fill: Rgb888,
    muted: Rgb888,
) {
    let available_width = canvas.width.saturating_sub(80);
    let card_width = available_width.saturating_sub(24) / 3;
    let card_height = 130_usize;
    for (index, line) in view.lines.iter().take(6).enumerate() {
        let column = index % 3;
        let row = index / 3;
        let x = 28 + column * (card_width + 12);
        let y = 122 + row * (card_height + 16);
        let stroke = if line.selected {
            bright.text_color.unwrap_or(muted)
        } else {
            muted
        };
        let style = PrimitiveStyleBuilder::new()
            .fill_color(if line.selected { selected_fill } else { panel })
            .stroke_color(stroke)
            .stroke_width(if line.selected { 3 } else { 1 })
            .build();
        Rectangle::new(
            Point::new(x as i32, y as i32),
            Size::new(card_width as u32, card_height as u32),
        )
        .into_styled(style)
        .draw(canvas)
        .ok();
        Text::new(
            &line.text,
            Point::new(x as i32 + 20, y as i32 + 68),
            if line.selected { bright } else { body },
        )
        .draw(canvas)
        .ok();
    }
}

fn render_linear(
    canvas: &mut Canvas<'_>,
    view: &NativeHomePresentation,
    body: MonoTextStyle<'_, Rgb888>,
    quiet: MonoTextStyle<'_, Rgb888>,
    panel: Rgb888,
    selected_fill: Rgb888,
) {
    let mut y = 122_i32;
    for line in view.lines.iter().take(24) {
        let height = if line.text.chars().count() > 100 {
            58
        } else {
            42
        };
        Rectangle::new(
            Point::new(28, y - 22),
            Size::new(canvas.width.saturating_sub(56) as u32, height as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(if line.selected {
            selected_fill
        } else {
            panel
        }))
        .draw(canvas)
        .ok();
        let style = if line.state == ApplicationNodeState::Unavailable {
            quiet
        } else {
            body
        };
        for row in wrap(&line.text, 135).into_iter().take(2) {
            Text::new(&row, Point::new(44, y), style).draw(canvas).ok();
            y += 14;
        }
        y += height - 14;
        if y > canvas.height as i32 - 90 {
            break;
        }
    }
}

fn truncate(text: &str, maximum: usize) -> String {
    text.chars().take(maximum).collect()
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for source in text.lines() {
        let mut remaining = source;
        while remaining.chars().count() > width {
            let boundary = remaining
                .char_indices()
                .nth(width)
                .map_or(remaining.len(), |(index, _)| index);
            let split = remaining[..boundary]
                .rfind(char::is_whitespace)
                .filter(|split| *split > 0)
                .unwrap_or(boundary);
            rows.push(remaining[..split].trim().to_owned());
            remaining = remaining[split..].trim_start();
        }
        rows.push(remaining.to_owned());
    }
    rows
}

impl OriginDimensions for Canvas<'_> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl DrawTarget for Canvas<'_> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let (x, y) = (point.x as usize, point.y as usize);
            if x < self.width && y < self.height {
                self.pixels[y * self.width + x] =
                    u32::from(color.r()) << 16 | u32::from(color.g()) << 8 | u32::from(color.b());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_preserves_unicode_boundaries() {
        let rows = wrap("Birth, spores, and the Crèche remain portable", 12);
        assert!(rows.len() > 1);
        assert!(rows.iter().all(|row| row.chars().count() <= 12));
        assert!(rows.concat().contains("Crèche"));
    }
}
