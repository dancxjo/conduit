//! Shared bounded proportional placement and coverage rasterization.
use super::{DisplayError, DisplayReceipt, PixelTarget, font, profile, put};
use conduit_presentation::{GraphicsCommand, GraphicsTextRole, LayoutRect};

pub(super) struct Positions<'a> {
    chars: core::str::Chars<'a>,
    role: GraphicsTextRole,
    width: u32,
    x: u32,
    pub y: u32,
    anchor: u32,
    word_start: bool,
}

impl<'a> Positions<'a> {
    fn new(value: &'a str, width: u16, role: GraphicsTextRole) -> Self {
        Self {
            chars: value.chars(),
            role,
            width: u32::from(width),
            x: 0,
            y: 0,
            anchor: 0,
            word_start: true,
        }
    }
    fn newline(&mut self) {
        self.x = 0;
        self.anchor = 0;
        self.y = self.y.saturating_add(profile::line_height(self.role));
    }
}

impl Iterator for Positions<'_> {
    type Item = (char, u32, u32);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let character = self.chars.next()?;
            if character == '\n' {
                self.newline();
                self.word_start = true;
                continue;
            }
            let advance = profile::advance(character, self.role);
            if self.word_start && !character.is_whitespace() {
                let mut word = advance;
                for next in self.chars.clone() {
                    if next.is_whitespace() || word > self.width {
                        break;
                    }
                    word = word.saturating_add(profile::advance(next, self.role));
                }
                if self.x > 0 && word <= self.width && self.x + word > self.width {
                    self.newline();
                }
            }
            self.word_start = character.is_whitespace();
            if self.x + advance > self.width && self.x > 0 {
                self.newline();
                if character.is_whitespace() {
                    continue;
                }
            }
            let x = if advance == 0 { self.anchor } else { self.x };
            if advance != 0 {
                self.anchor = self.x;
            }
            self.x = self.x.saturating_add(advance);
            return Some((character, x, self.y));
        }
    }
}

pub(super) fn height(value: &str, width: u16, role: GraphicsTextRole) -> Result<u16, DisplayError> {
    let mut positions = Positions::new(value, width, role);
    for (character, _, _) in positions.by_ref() {
        if profile::advance(character, role) > u32::from(width) {
            return Err(DisplayError::InvalidExtent);
        }
    }
    u16::try_from(positions.y + profile::line_height(role)).map_err(|_| DisplayError::InvalidExtent)
}

pub(super) fn draw(
    target: &mut impl PixelTarget,
    command: &GraphicsCommand,
    clip: LayoutRect,
    color: u32,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    let role = command.text_role;
    let color = match role {
        GraphicsTextRole::Muted => rgb(target, profile::MUTED),
        GraphicsTextRole::Status => rgb(target, profile::SUCCESS),
        GraphicsTextRole::Warning => rgb(target, profile::WARNING),
        _ => color,
    };
    for (character, x, y) in Positions::new(command.payload(), command.bounds.width, role) {
        if y >= u32::from(command.bounds.height) {
            break;
        }
        let origin_x = i32::from(command.bounds.x) + x as i32;
        let origin_y = i32::from(command.bounds.y) + y as i32;
        if let Some(glyph) = profile::glyph(character, role) {
            for row in 0..glyph.height {
                for column in 0..glyph.width {
                    let alpha = glyph.coverage[(row * glyph.width + column) as usize];
                    pixel(
                        target,
                        clip,
                        origin_x + glyph.bearing + column as i32,
                        origin_y + row as i32,
                        color,
                        alpha,
                        receipt,
                    )?;
                }
            }
        } else {
            // The exact existing admitted Unicode corpus is the deterministic secondary face.
            let (glyph, _) = font::glyph(character);
            for row in 0..font::GLYPH_HEIGHT {
                for column in 0..u32::from(glyph.width) {
                    let byte =
                        glyph.bitmap[(row * u32::from(glyph.width) / 8 + column / 8) as usize];
                    if byte & (0x80 >> (column % 8)) != 0 {
                        pixel(
                            target,
                            clip,
                            origin_x + column as i32,
                            origin_y + row as i32,
                            color,
                            255,
                            receipt,
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn rgb(target: &impl PixelTarget, (r, g, b): (u8, u8, u8)) -> u32 {
    target.format().pixel(r, g, b)
}

#[allow(clippy::too_many_arguments)]
fn pixel(
    target: &mut impl PixelTarget,
    clip: LayoutRect,
    x: i32,
    y: i32,
    color: u32,
    alpha: u8,
    receipt: &mut DisplayReceipt,
) -> Result<(), DisplayError> {
    if alpha == 0
        || x < i32::from(clip.x)
        || y < i32::from(clip.y)
        || x >= i32::from(clip.x) + i32::from(clip.width)
        || y >= i32::from(clip.y) + i32::from(clip.height)
    {
        return Ok(());
    }
    let background = target
        .read_pixel(x as u32, y as u32)
        .unwrap_or_else(|| rgb(target, profile::BACKGROUND));
    let format = target.format();
    let mut blended = 0;
    for shift in [format.red_shift, format.green_shift, format.blue_shift] {
        let fg = (color >> shift) & 255;
        let bg = (background >> shift) & 255;
        blended |= ((fg * u32::from(alpha) + bg * (255 - u32::from(alpha)) + 127) / 255) << shift;
    }
    put(target, x as u32, y as u32, blended, receipt)
}
