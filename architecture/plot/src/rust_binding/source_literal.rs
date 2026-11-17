//! Emit canonical bytes as one Rust literal instead of one AST node per byte.
use crate::prelude::*;

pub(super) fn byte_string_literal(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let capacity = bytes
        .iter()
        .enumerate()
        .fold(3usize, |size, (index, byte)| {
            size.saturating_add(match byte {
                0 if bytes.get(index + 1).is_some_and(u8::is_ascii_digit) => 4,
                b'"' | b'\\' | 0 | b'\t' | b'\r' | b'\n' => 2,
                0x20..=0x7e => 1,
                _ => 4,
            })
        });
    let mut literal = String::with_capacity(capacity);
    literal.push_str("b\"");
    for (index, &byte) in bytes.iter().enumerate() {
        match byte {
            b'"' => literal.push_str("\\\""),
            b'\\' => literal.push_str("\\\\"),
            // Keep NUL followed by a digit unambiguous to readers and Clippy.
            0 if bytes.get(index + 1).is_some_and(u8::is_ascii_digit) => {
                literal.push_str("\\x00");
            }
            0 => literal.push_str("\\0"),
            b'\t' => literal.push_str("\\t"),
            b'\r' => literal.push_str("\\r"),
            b'\n' => literal.push_str("\\n"),
            0x20..=0x7e => literal.push(char::from(byte)),
            _ => {
                literal.push_str("\\x");
                literal.push(HEX[usize::from(byte >> 4)] as char);
                literal.push(HEX[usize::from(byte & 15)] as char);
            }
        }
    }
    literal.push('"');
    literal
}

#[cfg(test)]
#[path = "source_literal/tests.rs"]
mod tests;
