use crate::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct SourceLine<'a> {
    pub(crate) text: &'a str,
    pub(crate) start: usize,
}

impl<'a> SourceLine<'a> {
    pub(crate) fn trimmed(self) -> (&'a str, usize) {
        let text = self.text.trim_start();
        (text.trim_end(), self.start + self.text.len() - text.len())
    }

    /// Return the statement portion of a source line. A `#` outside a quoted
    /// literal begins lossless CST trivia and is not part of surface grammar.
    pub(crate) fn statement(self) -> (&'a str, usize) {
        let (text, start) = self.trimmed();
        let end = comment_start(text).unwrap_or(text.len());
        (text[..end].trim_end(), start)
    }
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

pub(crate) fn top_level_positions(text: &str, target: char) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut delimiters = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in text.char_indices() {
        if let Some(active) = quote {
            if character == active && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            continue;
        }
        if character == target && delimiters.is_empty() {
            positions.push(offset);
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
