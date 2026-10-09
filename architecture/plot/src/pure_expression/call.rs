//! Semantic call and qualified variant-constructor parsing.
use super::*;
impl Parser<'_> {
    pub(super) fn atomic_or_call(
        &mut self,
        depth: usize,
    ) -> Result<ExpressionSyntax, (String, Span)> {
        let start = self.offset;
        let path = self.semantic_path_end().or_else(|| {
            crate::quantity_literal::compound_token_length(&self.text[start..])
                .map(|length| start + length)
        });
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
        if self.peek() == Some('.') && is_name(&self.text[start..self.offset]) {
            let tail = &self.text[self.offset + 1..];
            let case_length = tail
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
                .map(char::len_utf8)
                .sum::<usize>();
            if case_length > 0 && tail[case_length..].trim_start().starts_with('(') {
                self.offset += 1 + case_length;
            }
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
        } else if arguments.len() == 1
            && (is_name(&atom.text)
                || atom
                    .text
                    .split_once('.')
                    .is_some_and(|(owner, case)| is_name(owner) && is_name(case)))
        {
            Ok(ExpressionSyntax::Variant {
                tag: atom,
                payload: Box::new(arguments.pop().expect("one variant payload")),
                span,
            })
        } else {
            Err(("variant literal requires exactly one payload".into(), span))
        }
    }
}
