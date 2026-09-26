//! Bounded parser for canonical pure Conduitese expressions.

mod operator;

use crate::prelude::*;
use crate::surface_lex::{is_name, location};
use crate::syntax::{
    ExpressionProjection, ExpressionSyntax, SpannedText, StructuredExpressionField, UnaryOperator,
};
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
    let mut parser = Parser {
        source,
        text,
        source_start,
        offset: 0,
        nodes: 0,
    };
    let expression = parser.expression(0, 1)?;
    parser.whitespace();
    if parser.offset != text.len() {
        return Err(parser.error("unexpected trailing pure expression input"));
    }
    Ok(expression)
}

struct Parser<'a> {
    source: &'a str,
    text: &'a str,
    source_start: usize,
    offset: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn expression(
        &mut self,
        minimum: u8,
        depth: usize,
    ) -> Result<ExpressionSyntax, (String, Span)> {
        self.enter(depth)?;
        let mut left = self.prefix(depth)?;
        loop {
            self.whitespace();
            if minimum <= 1 && self.peek() == Some('?') {
                self.bump();
                let when_true = self.expression(0, depth)?;
                self.take(':')?;
                let when_false = self.expression(1, depth)?;
                let span = self.join(left.span(), when_false.span());
                left = ExpressionSyntax::Conditional {
                    condition: Box::new(left),
                    when_true: Box::new(when_true),
                    when_false: Box::new(when_false),
                    span,
                };
                continue;
            }
            let Some((operator, precedence, bytes)) = operator::binary(&self.text[self.offset..])
            else {
                break;
            };
            if precedence < minimum {
                break;
            }
            self.offset += bytes;
            let right = self.expression(precedence + 1, depth)?;
            let span = self.join(left.span(), right.span());
            left = ExpressionSyntax::Binary {
                operator,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn prefix(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        self.whitespace();
        let start = self.offset;
        let mut value = match self.peek() {
            Some('!') => {
                self.bump();
                let operand = self.expression(12, depth)?;
                ExpressionSyntax::Unary {
                    operator: UnaryOperator::Not,
                    span: self.from(start, operand.span().end - self.source_start),
                    operand: Box::new(operand),
                }
            }
            Some('-') => {
                self.bump();
                let operand = self.expression(12, depth)?;
                ExpressionSyntax::Unary {
                    operator: UnaryOperator::Negate,
                    span: self.from(start, operand.span().end - self.source_start),
                    operand: Box::new(operand),
                }
            }
            Some('(') => self.parenthesized(depth)?,
            Some('[') => self.collection(depth)?,
            Some('{') => self.record(depth)?,
            Some('.') => {
                self.bump();
                let input = ExpressionSyntax::Input(self.from(start, self.offset));
                let member_start = self.offset;
                while self.peek().is_some_and(|character| {
                    character.is_alphanumeric() || matches!(character, '_' | '-')
                }) {
                    self.bump();
                }
                if self.offset == member_start {
                    input
                } else {
                    let member = SpannedText {
                        text: self.text[member_start..self.offset].to_string(),
                        span: self.from(member_start, self.offset),
                    };
                    let projection = if member
                        .text
                        .chars()
                        .all(|character| character.is_ascii_digit())
                    {
                        ExpressionProjection::TupleIndex(member)
                    } else if is_name(&member.text) {
                        ExpressionProjection::Field(member)
                    } else {
                        return Err(("invalid input projection member".into(), member.span));
                    };
                    ExpressionSyntax::Projection {
                        value: Box::new(input),
                        member: projection,
                        span: self.from(start, self.offset),
                    }
                }
            }
            Some('\'') | Some('"') => self.quoted()?,
            Some(_) => self.atomic_or_call(depth)?,
            None => return Err(self.error("expected a pure expression value")),
        };
        loop {
            self.whitespace();
            if self.peek() != Some('.') {
                break;
            }
            self.bump();
            let member_start = self.offset;
            while self.peek().is_some_and(|character| {
                character.is_alphanumeric() || matches!(character, '_' | '-')
            }) {
                self.bump();
            }
            if self.offset == member_start {
                return Err(self.error("expected a field or tuple index after '.'"));
            }
            let member = SpannedText {
                text: self.text[member_start..self.offset].to_string(),
                span: self.from(member_start, self.offset),
            };
            let projection = if member
                .text
                .chars()
                .all(|character| character.is_ascii_digit())
            {
                ExpressionProjection::TupleIndex(member)
            } else if is_name(&member.text) {
                ExpressionProjection::Field(member)
            } else {
                return Err(("invalid projection member".into(), member.span));
            };
            let span = self.from(value.span().start - self.source_start, self.offset);
            value = ExpressionSyntax::Projection {
                value: Box::new(value),
                member: projection,
                span,
            };
        }
        Ok(value)
    }

    fn parenthesized(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        self.take('(')?;
        let first = self.expression(0, depth + 1)?;
        self.whitespace();
        if self.peek() != Some(',') {
            self.take(')')?;
            return Ok(first);
        }
        let mut values = vec![first];
        while self.peek() == Some(',') {
            self.bump();
            if values.len() == MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
                return Err(self.error("tuple exceeds the finite item limit"));
            }
            values.push(self.expression(0, depth + 1)?);
            self.whitespace();
        }
        self.take(')')?;
        Ok(ExpressionSyntax::Tuple {
            values,
            span: self.from(start, self.offset),
        })
    }

    fn collection(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        self.take('[')?;
        let mut values = Vec::new();
        self.whitespace();
        if self.peek() != Some(']') {
            loop {
                if values.len() == MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
                    return Err(self.error("structured collection exceeds the finite item limit"));
                }
                values.push(self.expression(0, depth + 1)?);
                self.whitespace();
                if self.peek() == Some(']') {
                    break;
                }
                self.take(',')?;
            }
        }
        self.take(']')?;
        Ok(ExpressionSyntax::Collection {
            values,
            span: self.from(start, self.offset),
        })
    }

    fn record(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        self.take('{')?;
        let mut fields = Vec::new();
        self.whitespace();
        if self.peek() == Some('}') {
            return Err(self.error("structured records must contain at least one field"));
        }
        loop {
            if fields.len() == MAXIMUM_STRUCTURED_RECORD_FIELDS {
                return Err(self.error("structured record exceeds the finite field limit"));
            }
            self.whitespace();
            let field_start = self.offset;
            let name = self.name("expected a structured record field name")?;
            self.whitespace();
            let (value, punned) = if self.peek() == Some(':') {
                self.bump();
                (self.expression(0, depth + 1)?, false)
            } else {
                (ExpressionSyntax::Atomic(name.clone()), true)
            };
            fields.push(StructuredExpressionField {
                name,
                span: self.from(field_start, value.span().end - self.source_start),
                value,
                punned,
            });
            self.whitespace();
            if self.peek() == Some('}') {
                break;
            }
            self.take(',')?;
        }
        self.take('}')?;
        Ok(ExpressionSyntax::Record {
            fields,
            span: self.from(start, self.offset),
        })
    }

    fn atomic_or_call(&mut self, depth: usize) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        let path = self.semantic_path_end();
        if let Some(end) = path {
            self.offset = end;
        } else {
            while let Some(character) = self.peek() {
                if character == '.' && self.decimal_point() {
                    self.bump();
                    continue;
                }
                if character == '%' && self.percentage_suffix(start) {
                    self.bump();
                    continue;
                }
                if operator::is_delimiter(character) {
                    break;
                }
                self.bump();
            }
        }
        if self.offset == start {
            return Err(self.error("expected an atomic expression value"));
        }
        let atom = SpannedText {
            text: self.text[start..self.offset].to_string(),
            span: self.from(start, self.offset),
        };
        self.whitespace();
        if self.peek() != Some('(') {
            return Ok(ExpressionSyntax::Atomic(atom));
        }
        self.bump();
        let mut arguments = Vec::new();
        self.whitespace();
        if self.peek() != Some(')') {
            loop {
                arguments.push(self.expression(0, depth + 1)?);
                self.whitespace();
                if self.peek() == Some(')') {
                    break;
                }
                self.take(',')?;
            }
        }
        self.take(')')?;
        let span = self.from(start, self.offset);
        if atom.text.contains('/') {
            Ok(ExpressionSyntax::SemanticCall {
                kind: atom,
                arguments,
                span,
            })
        } else if arguments.len() == 1 && is_name(&atom.text) {
            Ok(ExpressionSyntax::Variant {
                tag: atom,
                payload: Box::new(arguments.pop().expect("one variant payload")),
                span,
            })
        } else {
            Err(("variant literal requires exactly one payload".into(), span))
        }
    }

    fn percentage_suffix(&self, atom_start: usize) -> bool {
        self.text[atom_start..self.offset]
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_digit())
            && self.text[self.offset + 1..]
                .chars()
                .next()
                .is_none_or(|character| {
                    character.is_whitespace() || matches!(character, ')' | ']' | '}' | ',')
                })
    }

    fn quoted(&mut self) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        let quote = self.bump().expect("opening quote exists");
        let mut escaped = false;
        loop {
            let Some(character) = self.bump() else {
                return Err(self.error("unterminated quoted expression value"));
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
            span: self.from(start, self.offset),
        }))
    }

    fn semantic_path_end(&self) -> Option<usize> {
        let mut end = self.offset;
        let mut segments = 0;
        loop {
            let start = end;
            while self.text[end..].chars().next().is_some_and(|character| {
                character.is_alphanumeric() || matches!(character, '_' | '-')
            }) {
                end += self.text[end..].chars().next()?.len_utf8();
            }
            if end == start {
                return None;
            }
            segments += 1;
            if self.text[end..].starts_with('/') {
                end += 1;
                continue;
            }
            break;
        }
        let mut after = end;
        while self.text[after..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            after += self.text[after..].chars().next()?.len_utf8();
        }
        (segments >= 2 && self.text[after..].starts_with('(')).then_some(end)
    }

    fn decimal_point(&self) -> bool {
        self.text[..self.offset]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_digit())
            && self.text[self.offset + 1..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
    }

    fn name(&mut self, message: &str) -> Result<SpannedText, (String, Span)> {
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
        {
            self.bump();
        }
        let text = &self.text[start..self.offset];
        if !is_name(text) {
            return Err((message.into(), self.from(start, self.offset.max(start + 1))));
        }
        Ok(SpannedText {
            text: text.to_string(),
            span: self.from(start, self.offset),
        })
    }

    fn enter(&mut self, depth: usize) -> Result<(), (String, Span)> {
        if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(self.error("pure expression nesting exceeds the finite limit"));
        }
        self.nodes += 1;
        if self.nodes > MAXIMUM_STRUCTURED_INFO_NODES {
            return Err(self.error("pure expression contains too many values"));
        }
        Ok(())
    }

    fn take(&mut self, expected: char) -> Result<(), (String, Span)> {
        self.whitespace();
        if self.peek() != Some(expected) {
            return Err(self.error(&format!("expected '{expected}' in pure expression")));
        }
        self.bump();
        Ok(())
    }

    fn whitespace(&mut self) {
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

    fn join(&self, left: Span, right: Span) -> Span {
        self.from(
            left.start - self.source_start,
            right.end - self.source_start,
        )
    }

    fn from(&self, start: usize, end: usize) -> Span {
        let start = self.source_start + start;
        let end = self.source_start + end;
        let (line, column) = location(self.source, start);
        let (end_line, end_column) = location(self.source, end);
        Span {
            start,
            end,
            line,
            column,
            end_line,
            end_column,
        }
    }

    fn error(&self, message: &str) -> (String, Span) {
        let end = self
            .peek()
            .map_or(self.offset, |character| self.offset + character.len_utf8());
        (message.into(), self.from(self.offset, end))
    }
}
