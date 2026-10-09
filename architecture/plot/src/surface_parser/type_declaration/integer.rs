//! Bounded arithmetic parser for native Type extents.
use super::Parser;
use crate::prelude::*;
use crate::{
    NativeIntegerExpressionSyntax as Integer, NativeIntegerOperator as Operator, PlotError, Span,
};

type OperandParser<'a, 'b> = fn(&mut State<'a, 'b>, usize) -> Result<Integer, (PlotError, Span)>;

const MAXIMUM_BYTES: usize = 256;
const MAXIMUM_NODES: usize = 64;
const MAXIMUM_DEPTH: usize = 16;

pub(super) fn parse(
    parser: &Parser<'_>,
    source: &str,
    offset: usize,
) -> Result<Integer, (PlotError, Span)> {
    let mut state = State {
        parser,
        source,
        offset,
        cursor: 0,
        nodes: 0,
    };
    if source.len() > MAXIMUM_BYTES {
        return Err(state.error("native integer expression exceeds its byte limit"));
    }
    let value = state.sum(0)?;
    state.whitespace();
    if state.cursor != source.len() {
        return Err(state.error(
            "native integer expressions support only literals, parameters, +, * and parentheses",
        ));
    }
    Ok(value)
}

struct State<'a, 'b> {
    parser: &'a Parser<'b>,
    source: &'a str,
    offset: usize,
    cursor: usize,
    nodes: usize,
}

impl<'a, 'b> State<'a, 'b> {
    fn error(&self, message: &str) -> (PlotError, Span) {
        let end = self.source[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8());
        (
            PlotError::InvalidSyntax(message.into()),
            self.parser
                .span(self.offset + self.cursor, self.offset + end),
        )
    }

    fn whitespace(&mut self) {
        while let Some(c) = self.source[self.cursor..].chars().next() {
            if !c.is_whitespace() {
                break;
            }
            self.cursor += c.len_utf8();
        }
    }

    fn node(&mut self) -> Result<(), (PlotError, Span)> {
        self.nodes += 1;
        if self.nodes > MAXIMUM_NODES {
            return Err(self.error("native integer expression exceeds its node limit"));
        }
        Ok(())
    }

    fn sum(&mut self, depth: usize) -> Result<Integer, (PlotError, Span)> {
        self.binary(depth, b'+', Operator::Add, Self::product)
    }

    fn product(&mut self, depth: usize) -> Result<Integer, (PlotError, Span)> {
        self.binary(depth, b'*', Operator::Multiply, Self::atom)
    }

    fn binary(
        &mut self,
        depth: usize,
        token: u8,
        operator: Operator,
        operand: OperandParser<'a, 'b>,
    ) -> Result<Integer, (PlotError, Span)> {
        let mut left = operand(self, depth)?;
        loop {
            self.whitespace();
            if self.source.as_bytes().get(self.cursor) != Some(&token) {
                return Ok(left);
            }
            self.node()?;
            let operator_span = self
                .parser
                .span(self.offset + self.cursor, self.offset + self.cursor + 1);
            self.cursor += 1;
            let right = operand(self, depth)?;
            let span = self.parser.span(left.span().start, right.span().end);
            left = Integer::Binary {
                operator,
                left: Box::new(left),
                right: Box::new(right),
                operator_span,
                span,
            };
        }
    }

    fn atom(&mut self, depth: usize) -> Result<Integer, (PlotError, Span)> {
        self.whitespace();
        if depth >= MAXIMUM_DEPTH {
            return Err(self.error("native integer expression exceeds its nesting limit"));
        }
        self.node()?;
        let start = self.cursor;
        if self.source.as_bytes().get(start) == Some(&b'(') {
            self.cursor += 1;
            let value = self.sum(depth + 1)?;
            self.whitespace();
            if self.source.as_bytes().get(self.cursor) != Some(&b')') {
                return Err(self.error("native integer expression requires a closing parenthesis"));
            }
            self.cursor += 1;
            return Ok(Integer::Group {
                value: Box::new(value),
                span: self
                    .parser
                    .span(self.offset + start, self.offset + self.cursor),
            });
        }
        while let Some(c) = self.source[self.cursor..].chars().next() {
            if !c.is_alphanumeric() && c != '_' && c != '-' {
                break;
            }
            self.cursor += c.len_utf8();
        }
        let token = &self.source[start..self.cursor];
        let span = self
            .parser
            .span(self.offset + start, self.offset + self.cursor);
        if token.is_empty() {
            return Err(self.error("native integer expression requires a literal or parameter"));
        }
        if token.bytes().all(|c| c.is_ascii_digit()) {
            let value = token.parse::<u16>().map_err(|_| {
                (
                    PlotError::InvalidSyntax("native integer literal exceeds U16".into()),
                    span,
                )
            })?;
            return Ok(Integer::Literal { value, span });
        }
        if token.starts_with('-') || token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Err((
                PlotError::InvalidSyntax(
                    "native integer expression requires a nonnegative integer or parameter".into(),
                ),
                span,
            ));
        }
        Ok(Integer::Parameter(crate::SpannedText {
            text: token.into(),
            span,
        }))
    }
}

/// Parse an existing authored refinement operand, retaining its original origin.
pub(crate) fn parse_integer_spanned(
    source: &crate::SpannedText,
) -> Result<crate::NativeIntegerExpressionSyntax, (PlotError, Span)> {
    let parser = Parser::new(&source.text);
    let mut expression = parse(&parser, &source.text, 0)
        .map_err(|(error, span)| (error, relocate_span(span, source.span)))?;
    relocate_integer(&mut expression, source.span);
    Ok(expression)
}

fn relocate_span(span: Span, origin: Span) -> Span {
    Span {
        start: origin.start.saturating_add(span.start),
        end: origin.start.saturating_add(span.end),
        line: origin.line.saturating_add(span.line.saturating_sub(1)),
        column: if span.line == 1 {
            origin.column.saturating_add(span.column.saturating_sub(1))
        } else {
            span.column
        },
        end_line: origin.line.saturating_add(span.end_line.saturating_sub(1)),
        end_column: if span.end_line == 1 {
            origin
                .column
                .saturating_add(span.end_column.saturating_sub(1))
        } else {
            span.end_column
        },
    }
}

fn relocate_integer(value: &mut crate::NativeIntegerExpressionSyntax, origin: Span) {
    use crate::NativeIntegerExpressionSyntax as Integer;
    match value {
        Integer::Literal { span, .. } => *span = relocate_span(*span, origin),
        Integer::Parameter(name) => name.span = relocate_span(name.span, origin),
        Integer::Group { value, span } => {
            relocate_integer(value, origin);
            *span = relocate_span(*span, origin);
        }
        Integer::Binary {
            left,
            right,
            operator_span,
            span,
            ..
        } => {
            relocate_integer(left, origin);
            relocate_integer(right, origin);
            *operator_span = relocate_span(*operator_span, origin);
            *span = relocate_span(*span, origin);
        }
    }
}
