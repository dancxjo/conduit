//! Parsing for a behavior parameter's exact nested Fore.

use super::Parser;
use crate::syntax::FormFront;
use crate::{eof_span, FormError, Span};

impl Parser<'_> {
    pub(super) fn parse_kind_parameter_front(
        &mut self,
        open: usize,
    ) -> Result<FormFront, (FormError, Span)> {
        let mut front = FormFront::default();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text == ")" {
                front.span = Some(self.span(open, start + 1));
                self.index += 1;
                return Ok(front);
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
                continue;
            }
            if !text.contains(">>") {
                return Err((
                    FormError::InvalidSyntax(
                        "a kind parameter Fore contains only named runtime ports".into(),
                    ),
                    self.line_span(line),
                ));
            }
            self.parse_front_runtime(text, start, &mut front)?;
            self.index += 1;
        }
        Err((FormError::IncompleteForm, eof_span(self.source)))
    }
}
