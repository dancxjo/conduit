//! Authored physical declarations. The declaration body uses the ordinary
//! Conduitese expression AST; no secondary unit-definition language is parsed.

pub(crate) mod context;
pub use context::PhysicalUnitSourceOrigin;
mod admission;
mod parse;
mod registry;
mod scalar;
pub(crate) mod value;
pub use value::{checked_physical_catalog_for_document, parse_checked_physical_value};
mod syntax;
#[cfg(test)]
mod tests;

#[cfg(test)]
use admission::admit_declaration_graph;
pub(crate) use admission::admit_prefix_declarations;
pub(crate) use parse::{
    parse_dimension_declaration, parse_prefix_declaration, parse_quantity_definition,
    parse_unit_declaration,
};
pub(crate) use registry::{check_physical_declarations, CheckedPhysicalCatalogue};
pub(crate) use scalar::check_exact_scalar;
#[cfg(test)]
use scalar::ExactScalar;
pub use syntax::*;

use crate::prelude::*;
use crate::{Span, SpannedText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalDeclarationDiagnostic {
    pub message: String,
    pub span: Span,
}

pub(crate) fn refusal(span: Span, message: impl Into<String>) -> PhysicalDeclarationDiagnostic {
    PhysicalDeclarationDiagnostic {
        message: message.into(),
        span,
    }
}

pub(crate) fn symbol_allowed(symbol: &SpannedText) -> bool {
    !symbol.text.is_empty()
        && symbol.text.len() <= 64
        && !symbol.text.chars().any(|c| {
            c.is_whitespace()
                || c.is_control()
                || matches!(
                    c,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ':' | ',' | '=' | '#'
                )
        })
        && !symbol.text.starts_with(|c: char| c.is_ascii_digit())
        && symbol.text != "origin"
}
