//! Explicit ordinary Source locals as the constructor's typed literal context.
use crate::*;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue};

impl GlyphNotationScope {
    /// Select named immutable locals explicitly; names and context keys never
    /// select the literal parser or invent a missing domain basis.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_literal_from_source_locals<C: LiteralValueConstructor>(
        &self,
        document: &SyntaxDocument,
        plot_name: &str,
        literal: &TypedGlyphLiteralSyntax,
        bindings: &[(&str, &str)],
        constructor: &C,
        startup: &StartupCatalog,
        profile: &ProfileCatalog,
    ) -> Result<PreparedGlyphLiteral, LiteralPreparationRefusal<C::Refusal>> {
        use LiteralPreparationRefusal as R;
        let diagnostic = |span, message: &str| {
            R::SourceContext(SyntaxCheckDiagnostic {
                code: "CND-GLY-002",
                span,
                message: message.into(),
            })
        };
        if document.round_trip().len() > MAXIMUM_PLOT_SOURCE_BYTES
            || *document
                != parse_syntax_document_with_glyph_notations(document.round_trip(), startup)
            || !document.diagnostics.is_empty()
        {
            return Err(R::Source);
        }
        let fields = constructor.context_types();
        if fields.len() > 64 || bindings.len() != fields.len() {
            return Err(R::ContextLimit);
        }
        let mut plots = document
            .plots
            .iter()
            .filter(|plot| plot.name.text == plot_name);
        let plot = plots
            .next()
            .ok_or_else(|| diagnostic(literal.authored.span, "selected context Plot is missing"))?;
        if plots.next().is_some()
            || literal.authored.span.start < plot.span.start
            || literal.authored.span.end > plot.span.end
        {
            return Err(R::Source);
        }
        let mut locals = BTreeMap::new();
        for statement in &plot.back {
            if let BackStatement::LocalValue(local) = statement {
                if locals.insert(local.name.text.clone(), local).is_some() {
                    return Err(R::Source);
                }
            }
        }
        let empty = BTreeMap::new();
        let source_values = BTreeMap::new();
        let mut resolver = crate::syntax_check::Resolver::new(
            locals,
            plot.front
                .startup_parameters
                .iter()
                .map(|p| p.name.text.clone())
                .collect(),
            plot.front
                .runtime_ports
                .iter()
                .map(|p| p.name.text.clone())
                .collect(),
            BTreeSet::new(),
            &empty,
            &source_values,
        );
        let mut context = Vec::new();
        let mut bytes = 0usize;
        let mut keys = BTreeSet::new();
        for (key, ty) in fields {
            if !keys.insert(key.clone()) {
                return Err(R::ContextLimit);
            }
            let mut candidates = bindings.iter().filter(|(candidate, _)| *candidate == key);
            let (_, name) = candidates.next().ok_or_else(|| {
                diagnostic(
                    literal.authored.span,
                    "required Source context selection is missing",
                )
            })?;
            if candidates.next().is_some() {
                return Err(diagnostic(
                    literal.authored.span,
                    "Source context selection is ambiguous",
                ));
            }
            if !resolver.locals.contains_key(*name) {
                return Err(diagnostic(
                    literal.authored.span,
                    "selected immutable Source local is missing",
                ));
            }
            let span = resolver.locals[*name].value.span;
            let value = resolver
                .resolve_name(name, Some(&ty))
                .map_err(|error| R::SourceContext(error.diagnostic(span)))?;
            let CanonicalStartupValue::Structured(value) = value else {
                return Err(diagnostic(
                    span,
                    "Source context must be an exact concrete structured value",
                ));
            };
            let concrete = value.try_concrete().ok_or_else(|| {
                diagnostic(span, "Source context depends on an unbound parameter")
            })?;
            let encoded = concrete
                .canonical_bytes()
                .map_err(|_| diagnostic(span, "Source context exceeds canonical bounds"))?;
            bytes = bytes.saturating_add(encoded.len());
            if bytes > 1024 * 1024 {
                return Err(R::ContextLimit);
            }
            context.push(ConfigurationEntry {
                key,
                value: ConfigurationValue::Structured(
                    StructuredConfigurationValue::new(
                        ty.profile()
                            .map_err(|_| diagnostic(span, "Source context has no finite profile"))?
                            .value_kind()
                            .clone(),
                        encoded,
                    )
                    .ok_or_else(|| {
                        diagnostic(span, "Source context has an incompatible profile")
                    })?,
                ),
            });
        }
        let retained = resolver.resolved_context();
        if retained.len() > 64 {
            return Err(R::ContextLimit);
        }
        let mut bytes = 0usize;
        for (expression, value) in &retained {
            bytes = bytes.saturating_add(expression.text.len()).saturating_add(
                value
                    .try_concrete()
                    .ok_or(R::Source)?
                    .canonical_bytes()
                    .map_err(|_| R::ContextLimit)?
                    .len(),
            );
            if bytes > 1024 * 1024 {
                return Err(R::ContextLimit);
            }
        }
        let mut prepared =
            self.prepare_literal(document, literal, &context, constructor, startup, profile)?;
        prepared.source_context = retained;
        Ok(prepared)
    }
}
