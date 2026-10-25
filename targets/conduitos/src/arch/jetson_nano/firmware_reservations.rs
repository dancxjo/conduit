//! Additional CBoot command-line exclusions, not a complete firmware memory map.
//! The two size@address parameters describe retained firmware allocations, not a
//! pixel layout or permission to draw. Keep them excluded from general RAM.
//! NVIDIA's Tegra implementation parses both with memparse and reserves them:
//! https://android.googlesource.com/kernel/tegra/+/06f7cee4340d7ac777b11670e925893debb38239/arch/arm/mach-tegra/common.c
//! That source establishes parameter semantics, not stock Nano firmware coverage.

use super::{
    dtb::{Dtb, Token},
    memory::{Region, Regions},
};

const MAX_BOOTARGS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Dtb,
    Bounds,
    String,
    Duplicate,
    Number,
    Region,
    Capacity,
}

#[derive(Default, Debug)]
pub struct Reservations {
    pub framebuffer: Option<Region>,
    pub lp0_vector: Option<Region>,
}

impl Reservations {
    /// Add exclusions without interpreting either allocation as writable pixels.
    pub fn append_to(&self, reserved: &mut Regions) -> Result<(), Error> {
        for region in [self.framebuffer, self.lp0_vector].into_iter().flatten() {
            reserved.push(region, false).map_err(|_| Error::Capacity)?;
        }
        Ok(())
    }
}

fn number(mut text: &[u8]) -> Result<u64, Error> {
    let shift = match text.last() {
        Some(b'K' | b'k') => 10,
        Some(b'M' | b'm') => 20,
        Some(b'G' | b'g') => 30,
        _ => 0,
    };
    if shift != 0 {
        text = &text[..text.len() - 1];
    }
    let radix = if text.starts_with(b"0x") || text.starts_with(b"0X") {
        text = &text[2..];
        16
    } else {
        // Avoid Linux memparse's octal ambiguity: accept canonical decimal only.
        if text.len() > 1 && text[0] == b'0' {
            return Err(Error::Number);
        }
        10
    };
    if text.is_empty() {
        return Err(Error::Number);
    }
    let mut value = 0u64;
    for byte in text {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return Err(Error::Number),
        };
        if u64::from(digit) >= radix {
            return Err(Error::Number);
        }
        value = value
            .checked_mul(radix)
            .and_then(|v| v.checked_add(u64::from(digit)))
            .ok_or(Error::Number)?;
    }
    value.checked_mul(1u64 << shift).ok_or(Error::Number)
}

fn allocation(value: &[u8]) -> Result<Region, Error> {
    let separator = value.iter().position(|b| *b == b'@').ok_or(Error::Region)?;
    let size = number(&value[..separator])?;
    let address = number(&value[separator + 1..])?;
    Region::new(address, size).map_err(|_| Error::Region)
}

/// Parse a DT string (one trailing NUL required). Unknown ordinary parameters
/// confer no authority and are ignored; this function makes no completeness claim.
pub fn bootargs(value: &[u8]) -> Result<Reservations, Error> {
    if value.len() > MAX_BOOTARGS {
        return Err(Error::Bounds);
    }
    let value = value.strip_suffix(&[0]).ok_or(Error::String)?;
    if value.iter().any(|b| {
        !b.is_ascii()
            || (*b < 0x20 && !b.is_ascii_whitespace())
            || *b == 0x7f
            || matches!(*b, b'"' | b'\'')
    }) {
        return Err(Error::String);
    }
    let mut result = Reservations::default();
    for argument in value
        .split(|b| b.is_ascii_whitespace())
        .filter(|p| !p.is_empty())
    {
        let key_end = argument
            .iter()
            .position(|b| *b == b'=')
            .unwrap_or(argument.len());
        let target = match &argument[..key_end] {
            b"tegra_fbmem" => &mut result.framebuffer,
            b"lp0_vec" => &mut result.lp0_vector,
            _ => continue,
        };
        if target.is_some() {
            return Err(Error::Duplicate);
        }
        let value = argument.get(key_end + 1..).ok_or(Error::Region)?;
        *target = Some(allocation(value)?);
    }
    Ok(result)
}

/// Extract the single /chosen/bootargs string. Absent parameters remain absent,
/// never evidence that no retained allocation exists.
pub fn from_dtb(dtb: &Dtb<'_>) -> Result<Reservations, Error> {
    let mut tokens = dtb.tokens();
    let mut depth = 0;
    let mut chosen = false;
    let mut chosen_seen = false;
    let mut result = None;
    while let Some(token) = tokens.next_token().map_err(|_| Error::Dtb)? {
        match token {
            Token::Begin(name) => {
                depth += 1;
                if depth == 2 && name == b"chosen" {
                    if chosen_seen {
                        return Err(Error::Duplicate);
                    }
                    chosen_seen = true;
                    chosen = true;
                }
            }
            Token::Property {
                name: b"bootargs",
                value,
            } if chosen && depth == 2 => {
                if result.is_some() {
                    return Err(Error::Duplicate);
                }
                result = Some(bootargs(value)?);
            }
            Token::End => {
                if depth == 2 {
                    chosen = false;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    Ok(result.unwrap_or_default())
}

#[cfg(test)]
#[path = "firmware_reservations_tests.rs"]
mod tests;
