//! Surface declaration integration, staged separately from the physical value
//! checker. Bodies are parsed with the ordinary pure expression parser.
use super::Parser;
use crate::physical_declarations::{
    parse_dimension_declaration, parse_prefix_declaration, parse_quantity_definition,
    parse_unit_declaration, DimensionDeclarationSyntax, PrefixDeclarationSyntax,
    QuantityDefinitionSyntax, UnitDeclarationSyntax,
};
use crate::{PlotError, Span};

impl Parser<'_> {
    pub(super) fn parse_physical_prefix(
        &mut self,
    ) -> Result<PrefixDeclarationSyntax, (PlotError, Span)> {
        let (text, start) = self.lines[self.index].statement();
        let value = parse_prefix_declaration(self.source, text, start)
            .map_err(|error| (PlotError::InvalidSyntax(error.message), error.span))?;
        self.index += 1;
        Ok(value)
    }
    pub(super) fn parse_physical_dimension(
        &mut self,
    ) -> Result<DimensionDeclarationSyntax, (PlotError, Span)> {
        let (text, start) = self.lines[self.index].statement();
        let value = parse_dimension_declaration(self.source, text, start)
            .map_err(|error| (PlotError::InvalidSyntax(error.message), error.span))?;
        self.index += 1;
        Ok(value)
    }

    pub(super) fn parse_physical_unit(
        &mut self,
    ) -> Result<UnitDeclarationSyntax, (PlotError, Span)> {
        let (text, start) = self.lines[self.index].statement();
        let mut end = start + text.len();
        while !crate::surface_lex::delimiters_are_balanced(&self.source[start..end]) {
            self.index += 1;
            if self.index >= self.lines.len() {
                return Err((
                    PlotError::InvalidSyntax("unclosed physical unit declaration".into()),
                    self.span(start, end),
                ));
            }
            let (text, next) = self.lines[self.index].statement();
            end = next + text.len();
        }
        let value = parse_unit_declaration(self.source, &self.source[start..end], start)
            .map_err(|error| (PlotError::InvalidSyntax(error.message), error.span))?;
        self.index += 1;
        Ok(value)
    }

    /// Called by the ordinary `type NAME =` parser after it recognizes
    /// `quantity`. The resulting definition remains in the Type namespace.
    pub(super) fn parse_physical_quantity(
        &mut self,
        body: &str,
        header: &str,
        start: usize,
    ) -> Result<QuantityDefinitionSyntax, (PlotError, Span)> {
        let body = body
            .strip_prefix("quantity ")
            .ok_or_else(|| self.invalid_statement(header, start))?
            .trim();
        let offset = start
            + header
                .find(body)
                .ok_or_else(|| self.invalid_statement(header, start))?;
        let mut end = offset + body.len();
        while !crate::surface_lex::delimiters_are_balanced(&self.source[offset..end]) {
            self.index += 1;
            if self.index >= self.lines.len() {
                return Err((
                    PlotError::InvalidSyntax("unclosed quantity Type declaration".into()),
                    self.span(offset, end),
                ));
            }
            let (text, next) = self.lines[self.index].statement();
            end = next + text.len();
        }
        let expression =
            crate::pure_expression::parse(self.source, &self.source[offset..end], offset)
                .map_err(|(message, span)| (PlotError::InvalidSyntax(message), span))?;
        let value = parse_quantity_definition(&expression)
            .map_err(|error| (PlotError::InvalidSyntax(error.message), error.span))?;
        self.index += 1;
        Ok(value)
    }
}
