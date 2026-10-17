//! Emit canonical bytes as one Rust literal instead of one AST node per byte.
use crate::prelude::*;

pub(super) fn byte_string_literal(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut literal = String::with_capacity(bytes.len().saturating_mul(4).saturating_add(3));
    literal.push_str("b\"");
    for &byte in bytes {
        literal.push_str("\\x");
        literal.push(HEX[usize::from(byte >> 4)] as char);
        literal.push(HEX[usize::from(byte & 15)] as char);
    }
    literal.push('"');
    literal
}
