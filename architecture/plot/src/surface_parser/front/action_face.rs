//! Source declarations for checked Face action and fragment ports.

use super::*;
use crate::surface_lex::is_name;
use crate::syntax::ActionBindingSyntax;

impl Parser<'_> {
    pub(super) fn parse_action_face_directive(
        &self,
        text: &str,
        start: usize,
        front: &mut PlotFront,
    ) -> Result<bool, (PlotError, Span)> {
        if let Some(rest) = text.strip_prefix("action ") {
            let (declaration, input) = rest
                .split_once(">>")
                .ok_or_else(|| self.invalid_statement(text, start))?;
            let input = input.trim();
            let (intent, mapping) = declaration
                .trim()
                .split_once(char::is_whitespace)
                .ok_or_else(|| self.invalid_statement(text, start))?;
            if !is_name(input)
                || intent.is_empty()
                || !intent.chars().all(|ch| {
                    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '-' | '@' | '_')
                })
            {
                return Err(self.invalid_statement(text, start));
            }
            let (argument, value_type) = mapping
                .split_once(':')
                .ok_or_else(|| self.invalid_statement(text, start))?;
            let argument = argument.trim();
            let value_type = value_type.trim();
            if !is_name(argument) || !is_name(value_type) {
                return Err(self.invalid_statement(text, start));
            }
            front.action_bindings.push(ActionBindingSyntax {
                intent: self.spanned_at(intent, text, start),
                argument: self.spanned_at(argument, text, start),
                value_type: self.spanned_at(value_type, text, start),
                input: self.spanned_at(input, text, start),
                span: self.span(start, start + text.len()),
            });
            return Ok(true);
        }
        if let Some(output) = text.strip_prefix("face >>") {
            let output = output.trim();
            if !is_name(output) || front.face_fragment_output.is_some() {
                return Err(self.invalid_statement(text, start));
            }
            front.face_fragment_output = Some(self.spanned_at(output, text, start));
            return Ok(true);
        }
        Ok(false)
    }
}
