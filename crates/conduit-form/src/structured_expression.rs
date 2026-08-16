use crate::prelude::*;
use crate::surface_lex::{is_name, location};
use crate::syntax::{ExpressionSyntax, SpannedText, StructuredExpressionField};
use crate::Span;
use conduit_core::{
    MAXIMUM_STRUCTURED_COLLECTION_ITEMS, MAXIMUM_STRUCTURED_INFO_DEPTH,
    MAXIMUM_STRUCTURED_INFO_NODES, MAXIMUM_STRUCTURED_RECORD_FIELDS,
};

pub(crate) fn parse(
    source: &str,
    text: &str,
    source_start: usize,
) -> Result<ExpressionSyntax, (String, Span)> {
    if !looks_structured(text) {
        return Ok(ExpressionSyntax::Atomic(SpannedText {
            text: text.to_string(),
            span: source_span(source, source_start, source_start + text.len()),
        }));
    }
    let mut parser = Parser {
        source,
        text,
        source_start,
        offset: 0,
        nodes: 0,
    };
    let value = parser.parse_value(1)?;
    parser.skip_whitespace();
    if parser.offset != text.len() {
        return Err(parser.error("unexpected trailing structured expression input"));
    }
    Ok(value)
}

fn looks_structured(text: &str) -> bool {
    if text.starts_with(['[', '{']) {
        return true;
    }
    let tag_end = text
        .char_indices()
        .find(|(_, character)| !(character.is_alphanumeric() || matches!(character, '_' | '-')))
        .map_or(text.len(), |(offset, _)| offset);
    tag_end > 0 && text[tag_end..].trim_start().starts_with('(') && text.ends_with(')')
}

fn source_span(source: &str, start: usize, end: usize) -> Span {
    let (line, column) = location(source, start);
    let (end_line, end_column) = location(source, end);
    Span {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}

struct Parser<'a> {
    source: &'a str,
    text: &'a str,
    source_start: usize,
    offset: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn parse_value(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(self.error("structured expression nesting exceeds the finite limit"));
        }
        self.nodes += 1;
        if self.nodes > MAXIMUM_STRUCTURED_INFO_NODES {
            return Err(self.error("structured expression contains too many values"));
        }
        self.skip_whitespace();
        match self.peek() {
            Some('[') => self.parse_collection(depth),
            Some('{') => self.parse_record(depth),
            Some('\'') | Some('"') => self.parse_quoted(),
            Some(_) => self.parse_atomic_or_variant(depth),
            None => Err(self.error("expected a structured expression value")),
        }
    }

    fn parse_collection(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        self.take('[')?;
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.peek() != Some(']') {
            loop {
                if values.len() == MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
                    return Err(self.error("structured collection exceeds the finite item limit"));
                }
                values.push(self.parse_value(depth + 1)?);
                self.skip_whitespace();
                if self.peek() == Some(']') {
                    break;
                }
                self.take(',')?;
                self.skip_whitespace();
                if self.peek() == Some(']') {
                    return Err(self.error("structured collection has a trailing comma"));
                }
            }
        }
        self.take(']')?;
        Ok(ExpressionSyntax::Collection {
            values,
            span: self.span(start, self.offset),
        })
    }

    fn parse_record(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        self.take('{')?;
        let mut fields = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some('}') {
            return Err(self.error("structured records must contain at least one field"));
        }
        loop {
            if fields.len() == MAXIMUM_STRUCTURED_RECORD_FIELDS {
                return Err(self.error("structured record exceeds the finite field limit"));
            }
            self.skip_whitespace();
            let field_start = self.offset;
            let name = self.parse_name("expected a structured record field name")?;
            self.skip_whitespace();
            self.take(':')?;
            let value = self.parse_value(depth + 1)?;
            fields.push(StructuredExpressionField {
                name,
                span: self.span(field_start, value.span().end - self.source_start),
                value,
            });
            self.skip_whitespace();
            if self.peek() == Some('}') {
                break;
            }
            self.take(',')?;
            self.skip_whitespace();
            if self.peek() == Some('}') {
                return Err(self.error("structured record has a trailing comma"));
            }
        }
        self.take('}')?;
        Ok(ExpressionSyntax::Record {
            fields,
            span: self.span(start, self.offset),
        })
    }

    fn parse_atomic_or_variant(
        &mut self,
        depth: usize,
    ) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        while let Some(character) = self.peek() {
            if character.is_whitespace() || matches!(character, ',' | ']' | '}' | ':' | '(' | ')') {
                break;
            }
            self.bump();
        }
        if self.offset == start {
            return Err(self.error("expected an atomic value or variant tag"));
        }
        let token = &self.text[start..self.offset];
        self.skip_whitespace();
        if self.peek() == Some('(') {
            if !is_name(token) {
                return Err((
                    "invalid structured variant tag".into(),
                    self.span(start, self.offset),
                ));
            }
            let tag = SpannedText {
                text: token.to_string(),
                span: self.span(start, start + token.len()),
            };
            self.take('(')?;
            let payload = self.parse_value(depth + 1)?;
            self.skip_whitespace();
            self.take(')')?;
            Ok(ExpressionSyntax::Variant {
                tag,
                payload: Box::new(payload),
                span: self.span(start, self.offset),
            })
        } else {
            Ok(ExpressionSyntax::Atomic(SpannedText {
                text: token.to_string(),
                span: self.span(start, start + token.len()),
            }))
        }
    }

    fn parse_quoted(&mut self) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        let quote = self.bump().expect("quoted value has an opening character");
        let mut escaped = false;
        loop {
            let Some(character) = self.bump() else {
                return Err(self.error("unterminated quoted structured value"));
            };
            if character == quote && !escaped {
                break;
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
        }
        Ok(ExpressionSyntax::Atomic(SpannedText {
            text: self.text[start..self.offset].to_string(),
            span: self.span(start, self.offset),
        }))
    }

    fn parse_name(&mut self, message: &str) -> Result<SpannedText, (String, Span)> {
        let start = self.offset;
        while let Some(character) = self.peek() {
            if !(character.is_alphanumeric() || matches!(character, '_' | '-')) {
                break;
            }
            self.bump();
        }
        let text = &self.text[start..self.offset];
        if !is_name(text) {
            let end = self.text[start..]
                .chars()
                .next()
                .map_or(start, |character| start + character.len_utf8());
            return Err((message.into(), self.span(start, end)));
        }
        Ok(SpannedText {
            text: text.to_string(),
            span: self.span(start, self.offset),
        })
    }

    fn take(&mut self, expected: char) -> Result<(), (String, Span)> {
        self.skip_whitespace();
        if self.peek() != Some(expected) {
            return Err(self.error(&format!("expected '{expected}' in structured expression")));
        }
        self.bump();
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.text[self.offset..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    fn error(&self, message: &str) -> (String, Span) {
        let end = self.text[self.offset..]
            .chars()
            .next()
            .map_or(self.offset, |character| self.offset + character.len_utf8());
        (message.into(), self.span(self.offset, end))
    }

    fn span(&self, start: usize, end: usize) -> Span {
        let start = self.source_start + start;
        let end = self.source_start + end;
        source_span(self.source, start, end)
    }
}
