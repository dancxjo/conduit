//! Parsing for one compile-time startup value in a Form front.

use super::{
    canonical_default_bound, parse_port_type, split_declaration, split_default, split_type_bound,
    Parser,
};
use crate::syntax::{RuntimePortTemporal, StartupParameter};
use crate::{FormError, Span};

impl Parser<'_> {
    pub(super) fn parse_startup(
        &self,
        text: &str,
        start: usize,
    ) -> Result<StartupParameter, (FormError, Span)> {
        let (left, default) = split_default(text);
        let (name, value_type) =
            split_declaration(left).ok_or_else(|| self.invalid_statement(text, start))?;
        let span = self.span(start, start + text.len());
        let (value_type, refinements) = self.parse_value_refinements(value_type, text, start)?;
        let (value_type, explicit_bound) =
            split_type_bound(value_type).ok_or_else(|| self.invalid_statement(text, start))?;
        let (value_type, temporal) =
            parse_port_type(value_type).ok_or_else(|| self.invalid_statement(text, start))?;
        if !matches!(
            temporal,
            RuntimePortTemporal::Value | RuntimePortTemporal::OptionalValue
        ) {
            return Err(self.invalid_statement(text, start));
        }
        Ok(StartupParameter {
            name: self.spanned_at(name, text, start),
            value_type: self.spanned_at(value_type, text, start),
            optional: temporal == RuntimePortTemporal::OptionalValue,
            maximum_bytes: explicit_bound.or_else(|| canonical_default_bound(value_type)),
            refinements,
            default: default
                .map(|value| self.expression_at(value, text, start))
                .transpose()?,
            span,
        })
    }
}
