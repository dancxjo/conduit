//! Exact, target-independent fixed-width Conduitese integer literals.

use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerType {
    signed: bool,
    bits: u32,
}

pub(crate) fn canonicalize(literal: &str, value_kind: &str) -> Result<Option<String>, String> {
    let Some(integer_type) = integer_type(value_kind) else {
        return Ok(None);
    };
    let (negative, magnitude) = literal
        .strip_prefix('-')
        .map_or((false, literal), |magnitude| (true, magnitude));
    if negative && !integer_type.signed {
        return Err(format!(
            "negative literal is incompatible with unsigned {}-bit integer",
            integer_type.bits
        ));
    }
    let (radix, digits) = magnitude
        .strip_prefix("0x")
        .map(|digits| (16, digits))
        .or_else(|| magnitude.strip_prefix("0b").map(|digits| (2, digits)))
        .or_else(|| magnitude.strip_prefix("0o").map(|digits| (8, digits)))
        .unwrap_or((10, magnitude));
    let digits = separated_digits(digits, radix)?;
    let value = u128::from_str_radix(&digits, radix)
        .map_err(|_| "integer literal exceeds the portable 128-bit magnitude".to_string())?;
    let maximum = if integer_type.signed {
        if negative {
            1_u128 << (integer_type.bits - 1)
        } else {
            (1_u128 << (integer_type.bits - 1)) - 1
        }
    } else if integer_type.bits == 128 {
        u128::MAX
    } else {
        (1_u128 << integer_type.bits) - 1
    };
    if value > maximum {
        return Err(format!(
            "integer literal is outside the exact {}{} range",
            if integer_type.signed { "I" } else { "U" },
            integer_type.bits
        ));
    }
    Ok(Some(if negative && value != 0 {
        format!("-{value}")
    } else {
        value.to_string()
    }))
}

fn integer_type(value_kind: &str) -> Option<IntegerType> {
    let suffix = value_kind.strip_prefix("value/")?;
    let (signed, bits) = match suffix {
        "u8" => (false, 8),
        "u16" => (false, 16),
        "u32" => (false, 32),
        "u64" => (false, 64),
        "u128" => (false, 128),
        "i8" => (true, 8),
        "i16" => (true, 16),
        "i32" => (true, 32),
        "i64" => (true, 64),
        "i128" => (true, 128),
        _ => return None,
    };
    Some(IntegerType { signed, bits })
}

fn separated_digits(digits: &str, radix: u32) -> Result<String, String> {
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
    {
        return Err("integer separators must appear singly between digits".into());
    }
    let canonical = digits
        .chars()
        .filter(|character| *character != '_')
        .collect::<String>();
    if canonical
        .chars()
        .any(|character| character.to_digit(radix).is_none())
    {
        return Err(format!("integer literal contains a non-base-{radix} digit"));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_canonical_base_and_separator_normalizes_to_exact_meaning() {
        for literal in ["255", "0xff", "0b1111_1111", "0o377"] {
            assert_eq!(canonicalize(literal, "value/u8"), Ok(Some("255".into())));
        }
        assert_eq!(
            canonicalize("-0x8000_0000", "value/i32"),
            Ok(Some("-2147483648".into()))
        );
    }

    #[test]
    fn range_sign_and_separator_errors_refuse_portably() {
        for literal in ["256", "-1", "0x_1", "1_", "1__0", "0b2"] {
            assert!(canonicalize(literal, "value/u8").is_err(), "{literal}");
        }
        assert!(canonicalize("128", "value/i8").is_err());
        assert_eq!(canonicalize("-128", "value/i8"), Ok(Some("-128".into())));
    }
}
