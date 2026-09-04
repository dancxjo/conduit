//! Bounded 128x32 projection of ordinary portable Presentation text.

use conduit_presentation::{Presentation, PresentationError};

pub const OLED_LINE_BYTES: usize = 21;
pub const OLED_LINES: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OledLine {
    bytes: [u8; OLED_LINE_BYTES],
    len: u8,
}

impl OledLine {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ssd1306Frame {
    pub lines: [OledLine; OLED_LINES],
    pub framebuffer: [u8; conduit_ssd1306::FRAMEBUFFER_BYTES],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ssd1306ProjectionError {
    InvalidPresentation(PresentationError),
    NoDisplayableContent,
}

pub fn project_ssd1306_frame(
    presentation: &Presentation,
) -> Result<Ssd1306Frame, Ssd1306ProjectionError> {
    presentation
        .validate()
        .map_err(Ssd1306ProjectionError::InvalidPresentation)?;
    let mut candidates = presentation
        .text
        .iter()
        .map(|text| text.text.as_str())
        .chain(
            presentation
                .subjects
                .iter()
                .map(|subject| subject.label.as_str()),
        );
    let first = candidates
        .next()
        .ok_or(Ssd1306ProjectionError::NoDisplayableContent)?;
    let second = candidates.next().unwrap_or(presentation.identity.as_str());
    let lines = [line(first), line(second)];
    let mut framebuffer = [0_u8; conduit_ssd1306::FRAMEBUFFER_BYTES];
    render_line(&mut framebuffer, &lines[0], 0);
    render_line(&mut framebuffer, &lines[1], 16);
    Ok(Ssd1306Frame { lines, framebuffer })
}

fn line(source: &str) -> OledLine {
    let mut bytes = [b' '; OLED_LINE_BYTES];
    let mut len = 0;
    for byte in source.bytes() {
        if len == OLED_LINE_BYTES {
            break;
        }
        bytes[len] = match byte {
            b'a'..=b'z' => byte - 32,
            b' '..=b'~' => byte,
            _ => b'?',
        };
        len += 1;
    }
    OledLine {
        bytes,
        len: len as u8,
    }
}

fn render_line(
    framebuffer: &mut [u8; conduit_ssd1306::FRAMEBUFFER_BYTES],
    line: &OledLine,
    y: usize,
) {
    for (index, character) in line.as_bytes().iter().copied().enumerate() {
        let x = index * 6;
        for (column, bits) in glyph(character).into_iter().enumerate() {
            for row in 0..7 {
                if bits & (1 << row) != 0 {
                    set_pixel(framebuffer, x + column, y + row);
                }
            }
        }
    }
}

fn set_pixel(framebuffer: &mut [u8; conduit_ssd1306::FRAMEBUFFER_BYTES], x: usize, y: usize) {
    if x < conduit_ssd1306::WIDTH && y < conduit_ssd1306::HEIGHT {
        framebuffer[x + (y / 8) * conduit_ssd1306::WIDTH] |= 1 << (y % 8);
    }
}

#[rustfmt::skip]
fn glyph(character: u8) -> [u8; 5] {
    match character {
        b'A' => [0x7e,0x11,0x11,0x11,0x7e], b'B' => [0x7f,0x49,0x49,0x49,0x36],
        b'C' => [0x3e,0x41,0x41,0x41,0x22], b'D' => [0x7f,0x41,0x41,0x22,0x1c],
        b'E' => [0x7f,0x49,0x49,0x49,0x41], b'F' => [0x7f,0x09,0x09,0x09,0x01],
        b'G' => [0x3e,0x41,0x49,0x49,0x7a], b'H' => [0x7f,0x08,0x08,0x08,0x7f],
        b'I' => [0x00,0x41,0x7f,0x41,0x00], b'J' => [0x20,0x40,0x41,0x3f,0x01],
        b'K' => [0x7f,0x08,0x14,0x22,0x41], b'L' => [0x7f,0x40,0x40,0x40,0x40],
        b'M' => [0x7f,0x02,0x0c,0x02,0x7f], b'N' => [0x7f,0x04,0x08,0x10,0x7f],
        b'O' => [0x3e,0x41,0x41,0x41,0x3e], b'P' => [0x7f,0x09,0x09,0x09,0x06],
        b'Q' => [0x3e,0x41,0x51,0x21,0x5e], b'R' => [0x7f,0x09,0x19,0x29,0x46],
        b'S' => [0x46,0x49,0x49,0x49,0x31], b'T' => [0x01,0x01,0x7f,0x01,0x01],
        b'U' => [0x3f,0x40,0x40,0x40,0x3f], b'V' => [0x1f,0x20,0x40,0x20,0x1f],
        b'W' => [0x3f,0x40,0x38,0x40,0x3f], b'X' => [0x63,0x14,0x08,0x14,0x63],
        b'Y' => [0x07,0x08,0x70,0x08,0x07], b'Z' => [0x61,0x51,0x49,0x45,0x43],
        b'0' => [0x3e,0x51,0x49,0x45,0x3e], b'1' => [0x00,0x42,0x7f,0x40,0x00],
        b'2' => [0x42,0x61,0x51,0x49,0x46], b'3' => [0x21,0x41,0x45,0x4b,0x31],
        b'4' => [0x18,0x14,0x12,0x7f,0x10], b'5' => [0x27,0x45,0x45,0x45,0x39],
        b'6' => [0x3c,0x4a,0x49,0x49,0x30], b'7' => [0x01,0x71,0x09,0x05,0x03],
        b'8' => [0x36,0x49,0x49,0x49,0x36], b'9' => [0x06,0x49,0x49,0x29,0x1e],
        b' ' => [0;5], b'-' => [0x08,0x08,0x08,0x08,0x08], b'_' => [0x40;5],
        b'.' => [0x00,0x60,0x60,0x00,0x00], b':' => [0x00,0x36,0x36,0x00,0x00],
        b'/' => [0x20,0x10,0x08,0x04,0x02], b'%' => [0x62,0x64,0x08,0x13,0x23],
        _ => [0x7f,0x41,0x49,0x41,0x7f],
    }
}

#[cfg(test)]
#[path = "ssd1306_frame_tests.rs"]
pub(crate) mod tests;
