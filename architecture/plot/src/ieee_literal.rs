//! Contextual exact IEEE bit literals; integer hex contexts retain integers.
use crate::prelude::*;
pub(crate) fn f32_bits(literal: &str) -> Result<[u8; 4], String> {
    let digits = literal
        .strip_prefix("0x")
        .ok_or_else(|| String::from("F32 literal requires exact0xXXXXXXXX IEEE bits"))?;
    if digits.len() != 8 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("F32 literal requires exactly eight hexadecimal digits".into());
    }
    let bits =
        u32::from_str_radix(digits, 16).map_err(|_| String::from("invalid F32 bit literal"))?;
    Ok(bits.to_le_bytes())
}

pub(crate) fn f64_bits(literal: &str) -> Result<[u8; 8], String> {
    let digits = literal
        .strip_prefix("0x")
        .ok_or_else(|| String::from("F64 literal requires exact 0xXXXXXXXXXXXXXXXX IEEE bits"))?;
    if digits.len() != 16 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("F64 literal requires exactly sixteen hexadecimal digits".into());
    }
    let bits =
        u64::from_str_radix(digits, 16).map_err(|_| String::from("invalid F64 bit literal"))?;
    Ok(bits.to_le_bytes())
}
