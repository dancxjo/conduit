//! Allocator-free `std/text/format` reference semantics.
//!
//! The hosted runtime owns transport decoding and allocation. This module
//! owns the portable placeholder grammar, scalar rendering, finite limits,
//! and normalized failures.

/// Maximum UTF-8 bytes accepted in one template.
pub const FORMAT_MAX_TEMPLATE_BYTES: usize = 4 * 1024;
/// Maximum values accepted in one formatting collection.
pub const FORMAT_MAX_VALUES: usize = 32;
/// Maximum UTF-8 bytes accepted in one value name.
pub const FORMAT_MAX_NAME_BYTES: usize = 64;
/// Maximum UTF-8 bytes accepted in one text scalar.
pub const FORMAT_MAX_SCALAR_BYTES: usize = 1024;
/// Maximum UTF-8 bytes emitted for one formatted value.
pub const FORMAT_MAX_OUTPUT_BYTES: usize = 16 * 1024;
/// Maximum encoded bytes accepted on the `values` input.
pub const FORMAT_VALUES_MAX_ENCODED_BYTES: usize = 16 * 1024;
/// Maximum input bytes retained while the two finite inputs rendezvous.
pub const FORMAT_MAX_RETAINED_BYTES: usize =
    FORMAT_MAX_TEMPLATE_BYTES + FORMAT_VALUES_MAX_ENCODED_BYTES + FORMAT_MAX_OUTPUT_BYTES;
/// Conservative byte/comparison work ceiling for one formatter step.
pub const FORMAT_MAX_WORK: usize = FORMAT_MAX_TEMPLATE_BYTES
    + FORMAT_MAX_OUTPUT_BYTES
    + (FORMAT_MAX_TEMPLATE_BYTES / 2) * FORMAT_MAX_VALUES
    + FORMAT_MAX_VALUES * FORMAT_MAX_VALUES;

/// One explicitly supported scalar in `std/format-values`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatScalarRef<'a> {
    Text(&'a str),
    Boolean(bool),
    Integer(i128),
    /// A decoded future or corrupt kind. It fails closed.
    Unsupported(u8),
}

/// One ordered value with an optional unique name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatValueRef<'a> {
    pub name: Option<&'a str>,
    pub value: FormatScalarRef<'a>,
}

/// Stable normalized formatter outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatError {
    TemplateTooLarge,
    InvalidTextEncoding,
    TooManyValues,
    NameTooLarge,
    InvalidName,
    DuplicateName,
    ScalarTooLarge,
    MalformedPlaceholder,
    MissingValue,
    ExtraValue,
    UnsupportedValueKind,
    InvalidValuesEncoding,
    OutputOverflow,
}

impl FormatError {
    /// Stable provider-independent terminal code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TemplateTooLarge => "format/template-too-large",
            Self::InvalidTextEncoding => "format/invalid-text-encoding",
            Self::TooManyValues => "format/too-many-values",
            Self::NameTooLarge => "format/name-too-large",
            Self::InvalidName => "format/invalid-name",
            Self::DuplicateName => "format/duplicate-name",
            Self::ScalarTooLarge => "format/scalar-too-large",
            Self::MalformedPlaceholder => "format/malformed-placeholder",
            Self::MissingValue => "format/missing-value",
            Self::ExtraValue => "format/extra-value",
            Self::UnsupportedValueKind => "format/unsupported-value-kind",
            Self::InvalidValuesEncoding => "format/invalid-values-encoding",
            Self::OutputOverflow => "format/output-overflow",
        }
    }
}

/// Formats one finite template into caller-owned storage.
///
/// Grammar:
///
/// - `{}` consumes the next ordered value;
/// - `{0}` and later decimal indexes address collection order;
/// - `{name}` addresses a unique named value;
/// - `{{` and `}}` emit literal braces.
///
/// Every supplied value must be referenced at least once. Indexed and named
/// references may repeat, and an automatic placeholder advances independently
/// of explicit references.
pub fn format_text_into(
    template: &str,
    values: &[FormatValueRef<'_>],
    output: &mut [u8],
) -> Result<usize, FormatError> {
    if template.len() > FORMAT_MAX_TEMPLATE_BYTES {
        return Err(FormatError::TemplateTooLarge);
    }
    validate_format_values(values)?;
    let output_limit = output.len().min(FORMAT_MAX_OUTPUT_BYTES);
    let mut written = 0;
    let mut automatic_index = 0_usize;
    let mut used = [false; FORMAT_MAX_VALUES];
    let bytes = template.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' if bytes.get(cursor + 1) == Some(&b'{') => {
                write_bytes(b"{", output, output_limit, &mut written)?;
                cursor += 2;
            }
            b'}' if bytes.get(cursor + 1) == Some(&b'}') => {
                write_bytes(b"}", output, output_limit, &mut written)?;
                cursor += 2;
            }
            b'{' => {
                let start = cursor + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'}' {
                    if bytes[end] == b'{' {
                        return Err(FormatError::MalformedPlaceholder);
                    }
                    end += 1;
                }
                if end == bytes.len() {
                    return Err(FormatError::MalformedPlaceholder);
                }
                let placeholder = &template[start..end];
                let index = if placeholder.is_empty() {
                    let index = automatic_index;
                    automatic_index = automatic_index
                        .checked_add(1)
                        .ok_or(FormatError::MissingValue)?;
                    index
                } else if placeholder.as_bytes().iter().all(u8::is_ascii_digit) {
                    parse_index(placeholder)?
                } else if valid_name(placeholder) {
                    values
                        .iter()
                        .position(|value| value.name == Some(placeholder))
                        .ok_or(FormatError::MissingValue)?
                } else {
                    return Err(FormatError::MalformedPlaceholder);
                };
                let value = values.get(index).ok_or(FormatError::MissingValue)?;
                used[index] = true;
                write_scalar(value.value, output, output_limit, &mut written)?;
                cursor = end + 1;
            }
            b'}' => return Err(FormatError::MalformedPlaceholder),
            _ => {
                let next = bytes[cursor..]
                    .iter()
                    .position(|byte| matches!(byte, b'{' | b'}'))
                    .map_or(bytes.len(), |offset| cursor + offset);
                write_bytes(&bytes[cursor..next], output, output_limit, &mut written)?;
                cursor = next;
            }
        }
    }

    if used[..values.len()].iter().any(|used| !used) {
        return Err(FormatError::ExtraValue);
    }
    Ok(written)
}

/// Validates the exact bounded collection independently of a template.
pub fn validate_format_values(values: &[FormatValueRef<'_>]) -> Result<(), FormatError> {
    if values.len() > FORMAT_MAX_VALUES {
        return Err(FormatError::TooManyValues);
    }
    // Magic/version plus the one-byte entry count.
    let mut encoded_bytes = 5_usize;
    for (index, value) in values.iter().enumerate() {
        // Name length, name, and kind.
        encoded_bytes = encoded_bytes
            .checked_add(2)
            .and_then(|bytes| bytes.checked_add(value.name.map_or(0, str::len)))
            .ok_or(FormatError::InvalidValuesEncoding)?;
        if let Some(name) = value.name {
            if name.len() > FORMAT_MAX_NAME_BYTES {
                return Err(FormatError::NameTooLarge);
            }
            if !valid_name(name) {
                return Err(FormatError::InvalidName);
            }
            if values[..index].iter().any(|prior| prior.name == Some(name)) {
                return Err(FormatError::DuplicateName);
            }
        }
        match value.value {
            FormatScalarRef::Text(text) if text.len() > FORMAT_MAX_SCALAR_BYTES => {
                return Err(FormatError::ScalarTooLarge);
            }
            FormatScalarRef::Text(text) => {
                // Two-byte text length followed by UTF-8 bytes.
                encoded_bytes = encoded_bytes
                    .checked_add(2)
                    .and_then(|bytes| bytes.checked_add(text.len()))
                    .ok_or(FormatError::InvalidValuesEncoding)?;
            }
            FormatScalarRef::Boolean(_) => {
                encoded_bytes = encoded_bytes
                    .checked_add(1)
                    .ok_or(FormatError::InvalidValuesEncoding)?;
            }
            FormatScalarRef::Integer(_) => {
                encoded_bytes = encoded_bytes
                    .checked_add(16)
                    .ok_or(FormatError::InvalidValuesEncoding)?;
            }
            FormatScalarRef::Unsupported(_) => {
                return Err(FormatError::UnsupportedValueKind);
            }
        }
        if encoded_bytes > FORMAT_VALUES_MAX_ENCODED_BYTES {
            return Err(FormatError::InvalidValuesEncoding);
        }
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-'))
}

fn parse_index(value: &str) -> Result<usize, FormatError> {
    if value.len() > 1 && value.starts_with('0') {
        return Err(FormatError::MalformedPlaceholder);
    }
    value.bytes().try_fold(0_usize, |index, digit| {
        index
            .checked_mul(10)
            .and_then(|index| index.checked_add(usize::from(digit - b'0')))
            .ok_or(FormatError::MissingValue)
    })
}

fn write_scalar(
    value: FormatScalarRef<'_>,
    output: &mut [u8],
    output_limit: usize,
    written: &mut usize,
) -> Result<(), FormatError> {
    match value {
        FormatScalarRef::Text(value) => {
            write_bytes(value.as_bytes(), output, output_limit, written)
        }
        FormatScalarRef::Boolean(value) => write_bytes(
            if value { b"true" } else { b"false" },
            output,
            output_limit,
            written,
        ),
        FormatScalarRef::Integer(value) => {
            let mut digits = [0_u8; 40];
            let mut cursor = digits.len();
            let negative = value.is_negative();
            let mut magnitude = value.unsigned_abs();
            loop {
                cursor -= 1;
                digits[cursor] = b'0' + (magnitude % 10) as u8;
                magnitude /= 10;
                if magnitude == 0 {
                    break;
                }
            }
            if negative {
                cursor -= 1;
                digits[cursor] = b'-';
            }
            write_bytes(&digits[cursor..], output, output_limit, written)
        }
        FormatScalarRef::Unsupported(_) => Err(FormatError::UnsupportedValueKind),
    }
}

fn write_bytes(
    bytes: &[u8],
    output: &mut [u8],
    output_limit: usize,
    written: &mut usize,
) -> Result<(), FormatError> {
    let end = written
        .checked_add(bytes.len())
        .ok_or(FormatError::OutputOverflow)?;
    if end > output_limit {
        return Err(FormatError::OutputOverflow);
    }
    output[*written..end].copy_from_slice(bytes);
    *written = end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_named_automatic_and_escaped_placeholders_are_exact() {
        let values = [
            FormatValueRef {
                name: Some("worker"),
                value: FormatScalarRef::Text("alpha"),
            },
            FormatValueRef {
                name: Some("ready"),
                value: FormatScalarRef::Boolean(true),
            },
            FormatValueRef {
                name: None,
                value: FormatScalarRef::Integer(i128::MIN),
            },
        ];
        let mut output = [0; 256];
        let length =
            format_text_into("{worker}: {1}; {} / {2}; {{ok}}", &values, &mut output).unwrap();
        assert_eq!(
            core::str::from_utf8(&output[..length]).unwrap(),
            "alpha: true; alpha / -170141183460469231731687303715884105728; {ok}"
        );
    }

    #[test]
    fn normalized_failures_cover_contract_boundaries() {
        let one = [FormatValueRef {
            name: None,
            value: FormatScalarRef::Text("one"),
        }];
        let unsupported = [FormatValueRef {
            name: None,
            value: FormatScalarRef::Unsupported(9),
        }];
        let mut output = [0; 8];
        assert_eq!(
            format_text_into("{", &[], &mut output),
            Err(FormatError::MalformedPlaceholder)
        );
        assert_eq!(
            format_text_into("{}", &[], &mut output),
            Err(FormatError::MissingValue)
        );
        assert_eq!(
            format_text_into("", &one, &mut output),
            Err(FormatError::ExtraValue)
        );
        assert_eq!(
            format_text_into("{}", &unsupported, &mut output),
            Err(FormatError::UnsupportedValueKind)
        );
        assert_eq!(
            format_text_into("{}{}", &one, &mut output),
            Err(FormatError::MissingValue)
        );
        assert_eq!(
            format_text_into("{}", &one, &mut [0; 2]),
            Err(FormatError::OutputOverflow)
        );

        let moderate_scalar = [b'x'; 64];
        let moderate_scalar = core::str::from_utf8(&moderate_scalar).unwrap();
        let large = [FormatValueRef {
            name: None,
            value: FormatScalarRef::Text(moderate_scalar),
        }; FORMAT_MAX_VALUES];
        assert_eq!(validate_format_values(&large), Ok(()));

        let maximum_scalar = [b'x'; FORMAT_MAX_SCALAR_BYTES];
        let maximum_scalar = core::str::from_utf8(&maximum_scalar).unwrap();
        let maximum_scalars = [FormatValueRef {
            name: None,
            value: FormatScalarRef::Text(maximum_scalar),
        }; FORMAT_MAX_VALUES];
        assert_eq!(
            validate_format_values(&maximum_scalars),
            Err(FormatError::InvalidValuesEncoding)
        );
    }

    #[test]
    fn empty_and_maximum_output_boundaries_are_exact() {
        let mut empty = [0; 1];
        assert_eq!(format_text_into("", &[], &mut empty), Ok(0));

        let scalar = [b'x'; FORMAT_MAX_SCALAR_BYTES];
        let scalar = core::str::from_utf8(&scalar).unwrap();
        let values = [FormatValueRef {
            name: None,
            value: FormatScalarRef::Text(scalar),
        }];
        let mut maximum = [0; FORMAT_MAX_OUTPUT_BYTES];
        let template = "{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}";
        assert_eq!(
            format_text_into(template, &values, &mut maximum),
            Ok(FORMAT_MAX_OUTPUT_BYTES)
        );
        let overflow = "{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}{0}";
        assert_eq!(
            format_text_into(overflow, &values, &mut maximum),
            Err(FormatError::OutputOverflow)
        );
    }
}
