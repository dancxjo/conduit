//! One document's explicit lexical family bindings, never global aliases.
use crate::prelude::*;
use crate::{
    BackStatement, PlotSyntax, ScannedTypedLiteral, StartupCatalog, SyntaxCheckDiagnostic,
    SyntaxDocument, TypedLiteralFamily, TypedLiteralScanRefusal,
};
use alloc::collections::{BTreeMap, BTreeSet};

const MAXIMUM_SCOPE_BYTES: usize = 1024 * 1024;
const MAXIMUM_ALIAS_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedGlyphNotation {
    pub context: Vec<(crate::SpannedText, crate::SpannedText)>,
    pub alias: String,
    pub source_path: String,
    pub family: TypedLiteralFamily,
    pub import_span: crate::Span,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlyphNotationScope {
    bindings: BTreeMap<String, ScopedGlyphNotation>,
}

impl GlyphNotationScope {
    pub fn binding(&self, alias: &str) -> Option<&ScopedGlyphNotation> {
        self.bindings.get(alias)
    }

    pub fn bindings(&self) -> impl Iterator<Item = &ScopedGlyphNotation> {
        self.bindings.values()
    }

    /// Scan only a family explicitly bound in this document, without result-Type
    /// selection, escape decoding, payload admission or implicit imports.
    pub fn scan_literal<'a>(
        &'a self,
        alias: &str,
        source: &'a str,
    ) -> Result<ScannedTypedLiteral<'a>, TypedLiteralScanRefusal> {
        self.binding(alias)
            .ok_or(TypedLiteralScanRefusal::UnboundPrefix)?
            .family
            .scan_literal(alias, source)
    }

    /// Parse a bounded expression using only this document's resolved families.
    /// Recognition retains lexical candidates; it does not admit domain Info.
    pub fn parse_expression(
        &self,
        source: &str,
        span: crate::Span,
    ) -> Result<crate::Expression, (String, crate::Span)> {
        let text = source.get(span.start..span.end).ok_or_else(|| {
            (
                "expression span is outside the authored UTF-8 source".into(),
                span,
            )
        })?;
        let syntax =
            crate::pure_expression::parse_with_scope(source, text, span.start, Some(self))?;
        let (line, column) = crate::surface_lex::location(source, span.start);
        let (end_line, end_column) = crate::surface_lex::location(source, span.end);
        Ok(crate::Expression {
            text: text.into(),
            syntax,
            span: crate::Span {
                line,
                column,
                end_line,
                end_column,
                ..span
            },
        })
    }

    pub(crate) fn require_used(
        &self,
        used: &BTreeSet<String>,
    ) -> Result<(), SyntaxCheckDiagnostic> {
        if let Some(binding) = self
            .bindings
            .values()
            .find(|binding| !used.contains(&binding.alias))
        {
            return Err(error(
                binding.import_span,
                format!("unused with glyph notation alias '{}'", binding.alias),
            ));
        }
        Ok(())
    }
}

/// Resolve only checked shipped family paths. Other import categories remain
/// owned by their ordinary resolvers. Nothing is installed into the shared
/// catalogue, so distinct modules may bind different families under `ph`.
pub fn resolve_glyph_notation_scope(
    document: &SyntaxDocument,
    startup: &StartupCatalog,
) -> Result<GlyphNotationScope, SyntaxCheckDiagnostic> {
    if let Some(diagnostic) = document.diagnostics.first() {
        return Err(SyntaxCheckDiagnostic {
            code: diagnostic.code,
            span: diagnostic.span,
            message: diagnostic.message.clone(),
        });
    }
    let mut scope = GlyphNotationScope::default();
    let mut bytes = 0usize;
    for import in &document.uses {
        let Some(family) = startup.typed_literal_family(&import.path) else {
            if !import.glyph_context.is_empty() {
                return Err(error(
                    import.span,
                    "using context requires a checked glyph notation import",
                ));
            }
            continue;
        };
        let alias = &import.alias.text;
        if alias.len() > MAXIMUM_ALIAS_BYTES
            || !crate::surface_lex::is_name(alias)
            || !alias
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            return Err(error(import.alias.span, "glyph notation alias must be a finite identifier beginning with a letter or underscore"));
        }
        let mut plot_shadow = false;
        for plot in &document.plots {
            plot_shadow |= plot_conflicts(plot, alias, 1)?;
        }
        if document
            .uses
            .iter()
            .filter(|other| other.alias.text == *alias)
            .count()
            != 1
            || startup.signature(alias).is_some()
            || startup.structured_type(alias).is_some()
            || startup.value_kind_alias(alias).is_some()
            || startup.native_families.contains_key(alias)
            || startup.typed_literal_family(alias).is_some()
            || document.types.iter().any(|ty| ty.name.text == *alias)
            || document
                .type_forms
                .iter()
                .any(|form| form.name.text == *alias)
            || document
                .glyph_notations
                .iter()
                .any(|notation| notation.name.text == *alias)
            || plot_shadow
            || document.constructions.iter().any(|construction| {
                construction.name.text == *alias
                    || construction
                        .declarations
                        .iter()
                        .any(|value| value.name.text == *alias)
            })
        {
            return Err(error(
                import.alias.span,
                format!("glyph notation alias '{alias}' conflicts with another lexical binding"),
            ));
        }
        let identity = family
            .identity_bytes()
            .map_err(|message| error(import.path_span, message))?;
        bytes = bytes
            .saturating_add(alias.len())
            .saturating_add(import.path.len())
            .saturating_add(identity.len())
            .saturating_add(
                import
                    .glyph_context
                    .iter()
                    .map(|(key, local)| key.text.len() + local.text.len())
                    .sum::<usize>(),
            );
        if scope.bindings.len() >= crate::MAXIMUM_TYPED_LITERAL_FAMILIES
            || bytes > MAXIMUM_SCOPE_BYTES
        {
            return Err(error(
                import.span,
                "glyph notation scope exceeds its finite family or identity-byte bound",
            ));
        }
        scope.bindings.insert(
            alias.clone(),
            ScopedGlyphNotation {
                context: import.glyph_context.clone(),
                alias: alias.clone(),
                source_path: import.path.clone(),
                family: family.clone(),
                import_span: import.span,
            },
        );
    }
    Ok(scope)
}

fn plot_conflicts(
    plot: &PlotSyntax,
    alias: &str,
    depth: usize,
) -> Result<bool, SyntaxCheckDiagnostic> {
    if depth > crate::MAXIMUM_PLOT_NESTING_DEPTH {
        return Err(error(
            plot.span,
            "glyph notation scope exceeds the existing finite Plot nesting bound",
        ));
    }
    let mut nested_shadow = false;
    for local in &plot.local_plots {
        nested_shadow |= plot_conflicts(local, alias, depth + 1)?;
    }
    Ok(plot.name.text == alias
        || plot
            .front
            .type_parameters
            .iter()
            .any(|value| value.name.text == alias)
        || plot
            .front
            .kind_parameters
            .iter()
            .any(|value| value.name.text == alias)
        || plot
            .front
            .startup_parameters
            .iter()
            .any(|value| value.name.text == alias)
        || plot
            .front
            .runtime_ports
            .iter()
            .any(|value| value.name.text == alias)
        || plot.back.iter().any(|statement| match statement {
            BackStatement::NamedGear(value) => value.name.text == alias,
            BackStatement::LocalValue(value) => value.name.text == alias,
            BackStatement::Pool(value) => value.name.text == alias,
            BackStatement::Cord(_) | BackStatement::MatchedRoute(_) => false,
        })
        || nested_shadow)
}

fn error(span: crate::Span, message: impl Into<String>) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-062",
        span,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
