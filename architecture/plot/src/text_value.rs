use crate::prelude::*;

pub(crate) fn parse_quoted_text(source: &str) -> Option<String> {
    parse_quoted_text_mapped(source, usize::MAX, |_, _| {})
}

pub(crate) fn parse_quoted_text_mapped(
    source: &str,
    maximum: usize,
    mut mapped: impl FnMut(core::ops::Range<usize>, core::ops::Range<usize>),
) -> Option<String> {
    let body = source.strip_prefix('"')?.strip_suffix('"')?;
    let mut decoded = String::with_capacity(body.len().min(maximum));
    let mut chars = body.char_indices();
    while let Some((start, character)) = chars.next() {
        let (character, end) = if character == '\\' {
            let (offset, escaped) = chars.next()?;
            let character = match escaped {
                '"' => '"',
                '\\' => '\\',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => return None,
            };
            (character, offset + escaped.len_utf8())
        } else {
            (character, start + character.len_utf8())
        };
        let decoded_start = decoded.len();
        if decoded_start.checked_add(character.len_utf8())? > maximum {
            return None;
        }
        decoded.push(character);
        mapped(decoded_start..decoded.len(), start + 1..end + 1);
    }
    Some(decoded)
}

/// Quote one immutable Text startup value using the syntax's retained escape form.
pub fn text_startup_literal(value: &str) -> String {
    let mut literal = String::with_capacity(value.len() + 2);
    literal.push('"');
    for character in value.chars() {
        match character {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            _ => literal.push(character),
        }
    }
    literal.push('"');
    literal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_startup_quoting_preserves_unicode_and_each_retained_escape() {
        let value = "雪\"\\\n\r\t\0\u{8}\u{c}";
        assert_eq!(
            parse_quoted_text(&text_startup_literal(value)).as_deref(),
            Some(value)
        );
    }
}
