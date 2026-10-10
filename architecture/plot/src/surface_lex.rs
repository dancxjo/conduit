use crate::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct SourceLine<'a> {
    pub(crate) text: &'a str,
    pub(crate) start: usize,
    pub(crate) statement_end: usize,
}

impl<'a> SourceLine<'a> {
    /// Return the statement portion using its prepared comment boundary.
    /// Scoped parsing shields declared glyph payloads before setting this end.
    pub(crate) fn statement(self) -> (&'a str, usize) {
        let text = &self.text[..self.statement_end];
        let trimmed = text.trim_start();
        (trimmed.trim_end(), self.start + text.len() - trimmed.len())
    }
}

pub(crate) fn comment_start_with_scope(
    text: &str,
    scope: &crate::GlyphNotationScope,
) -> Option<usize> {
    let mut offset = 0;
    let mut quote = None;
    let mut escaped = false;
    while offset < text.len() {
        if quote.is_none() {
            if let Some(literal) = scope.scan_at(text, offset) {
                offset += literal.consumed_bytes;
                continue;
            }
        }
        let character = text[offset..].chars().next().unwrap();
        if let Some(active) = quote {
            if character == active && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
        } else {
            match character {
                '\'' | '"' => quote = Some(character),
                '#' => return Some(offset),
                _ => {}
            }
        }
        offset += character.len_utf8();
    }
    None
}

pub(crate) fn comment_start(text: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in text.char_indices() {
        if let Some(active) = quote {
            if character == active && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '#' => return Some(offset),
            _ => {}
        }
    }
    None
}

pub(crate) fn split_declaration(text: &str) -> Option<(&str, &str)> {
    let colon = top_level_positions(text, ':').first().copied()?;
    let name = text[..colon].trim();
    let value_type = text[colon + 1..].trim();
    (is_name(name) && !value_type.is_empty()).then_some((name, value_type))
}

pub(crate) fn split_top_level_once(text: &str, delimiter: char) -> (&str, Option<&str>) {
    top_level_positions(text, delimiter)
        .first()
        .map_or((text, None), |position| {
            (
                &text[..*position],
                Some(&text[*position + delimiter.len_utf8()..]),
            )
        })
}

pub(crate) fn split_top_level(text: &str, delimiter: char) -> Vec<&str> {
    let positions = top_level_positions(text, delimiter);
    let mut result = Vec::new();
    let mut start = 0;
    for position in positions {
        result.push(&text[start..position]);
        start = position + delimiter.len_utf8();
    }
    result.push(&text[start..]);
    result
}

pub(crate) fn split_top_level_token<'a>(text: &'a str, token: &str) -> Vec<&'a str> {
    let positions = top_level_token_positions(text, token);
    let mut result = Vec::new();
    let mut start = 0;
    for position in positions {
        result.push(&text[start..position]);
        start = position + token.len();
    }
    result.push(&text[start..]);
    result
}

pub(crate) fn top_level_token_positions(text: &str, token: &str) -> Vec<usize> {
    if token.is_empty() {
        return Vec::new();
    }
    top_level_positions(text, token.chars().next().unwrap())
        .into_iter()
        .filter(|position| text[*position..].starts_with(token))
        .collect()
}

pub(crate) fn top_level_positions(text: &str, target: char) -> Vec<usize> {
    top_level_positions_in_scope(text, target, None)
}

pub(crate) fn top_level_positions_in_scope(
    text: &str,
    target: char,
    scope: Option<&crate::GlyphNotationScope>,
) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut delimiters = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut offset = 0;
    while offset < text.len() {
        if quote.is_none() {
            if let Some(literal) = scope.and_then(|scope| scope.scan_at(text, offset)) {
                offset += literal.consumed_bytes;
                continue;
            }
        }
        let position = offset;
        let character = text[offset..].chars().next().unwrap();
        offset += character.len_utf8();
        if let Some(active) = quote {
            if character == active && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            continue;
        }
        if character == target && delimiters.is_empty() {
            positions.push(position);
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => delimiters.push(')'),
            '[' => delimiters.push(']'),
            '{' => delimiters.push('}'),
            ')' | ']' | '}' if delimiters.last() == Some(&character) => {
                delimiters.pop();
            }
            _ => {}
        }
    }
    positions
}

pub(crate) fn delimiters_are_balanced(text: &str) -> bool {
    let mut delimiters = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for character in text.chars() {
        if let Some(active) = quote {
            if character == active && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => delimiters.push(')'),
            '[' => delimiters.push(']'),
            '{' => delimiters.push('}'),
            ')' | ']' | '}' if delimiters.pop() != Some(character) => return false,
            _ => {}
        }
    }
    delimiters.is_empty() && quote.is_none()
}

pub(crate) fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
}

pub(crate) fn is_reference(text: &str) -> bool {
    text.split('.').all(is_name)
}

pub(crate) fn is_operation(text: &str) -> bool {
    text.split('/').all(is_name)
}

pub(crate) fn is_source_import_path(text: &str) -> bool {
    is_operation(text)
        || text
            .strip_prefix("./")
            .is_some_and(|relative| relative.contains('/') && is_operation(relative))
}

/// A punctuation-only lexical Gear name. Core grammar punctuation remains
/// unavailable as a binding, so a glyph can never redefine cord, terminal,
/// cancellation, route, grouping, or completion syntax.
pub(crate) fn is_glyph(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 8
        && text
            .chars()
            .all(|character| matches!(character, '<' | '>' | '&' | '?' | '@' | '^'))
        && !text.contains(">>")
        && !matches!(text, ">>" | ">" | "|" | "!" | "~" | "?" | ".")
}

pub(crate) fn is_gear_name(text: &str) -> bool {
    is_operation(text) || is_glyph(text)
}

pub(crate) fn location(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, tail)| tail)
        .chars()
        .count()
        + 1;
    (line, column)
}
