//! Explicit ordinary Source locals as the constructor's typed literal context.
use crate::*;
mod validation;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue};

impl GlyphNotationScope {
    /// Prepare with the explicit `using {key: local}` selection retained by
    /// this literal's authored glyph import.
    pub fn prepare_literal_from_authored_context<C: LiteralValueConstructor>(
        &self,
        document: &SyntaxDocument,
        plot_name: &str,
        literal: &TypedGlyphLiteralSyntax,
        constructor: &C,
        startup: &StartupCatalog,
        profile: &ProfileCatalog,
    ) -> Result<PreparedGlyphLiteral, LiteralPreparationRefusal<C::Refusal>> {
        let binding = self
            .binding(&literal.alias.text)
            .ok_or(LiteralPreparationRefusal::Identity)?;
        let fresh = resolve_glyph_notation_scope(document, startup)
            .map_err(LiteralPreparationRefusal::SourceContext)?;
        if fresh.binding(&literal.alias.text) != Some(binding) {
            return Err(LiteralPreparationRefusal::Identity);
        }
        let fail = |message: &str| {
            LiteralPreparationRefusal::SourceContext(SyntaxCheckDiagnostic {
                code: "CND-GLY-002",
                span: binding.import_span,
                message: message.into(),
            })
        };
        let mut known = BTreeMap::new();
        let mut fields = constructor.context_types();
        if fields.len() > 64 {
            return Err(LiteralPreparationRefusal::ContextLimit);
        }
        for (key, ty) in &fields {
            known.insert(key.clone(), ty.clone());
        }
        for branch in &binding.family.branches {
            let Some(owner) = startup.literal_owners.get(branch.constructor_kind.as_str()) else {
                continue;
            };
            let owner_fields = owner.context_types();
            if owner_fields.len() > 64 {
                return Err(LiteralPreparationRefusal::ContextLimit);
            }
            for (key, ty) in owner_fields {
                if known.get(&key).is_some_and(|existing| existing != &ty) {
                    return Err(fail(
                        "glyph branches declare incompatible Types for a shared context key",
                    ));
                }
                known.insert(key, ty);
                if known.len() > 64 {
                    return Err(LiteralPreparationRefusal::ContextLimit);
                }
            }
        }
        for (key, _) in &binding.context {
            let ty = known.get(&key.text).ok_or_else(|| fail("selected context key is not declared by this glyph family's compiled constructors"))?;
            if !fields.iter().any(|(required, _)| *required == key.text) {
                fields.push((key.text.clone(), ty.clone()));
            }
        }
        if fields.is_empty() && binding.context.is_empty() {
            return self.prepare_literal(document, literal, &[], constructor, startup, profile);
        }
        let constructor = FamilyContextConstructor {
            owner: constructor,
            fields,
        };
        let selections = binding
            .context
            .iter()
            .map(|(key, local)| (key.text.as_str(), local.text.as_str()))
            .collect::<Vec<_>>();
        self.prepare_literal_from_source_locals(
            document,
            plot_name,
            literal,
            &selections,
            &constructor,
            startup,
            profile,
        )
    }

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
        resolver.bound_glyph_context();
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
            validation::validate(&concrete, startup)
                .map_err(|message| diagnostic(span, &message))?;
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
        let retained = resolver.resolved_context().ok_or(R::ContextLimit)?;
        let mut prepared =
            self.prepare_literal(document, literal, &context, constructor, startup, profile)?;
        prepared.source_context = retained;
        Ok(prepared)
    }
}

/// Check all selected family context before projecting the exact configuration
/// required by this literal's declared constructor branch.
struct FamilyContextConstructor<'a, C> {
    owner: &'a C,
    fields: Vec<(String, conduit_core::StructuredInfoType)>,
}
impl<C: LiteralValueConstructor> StaticValueConstructor for FamilyContextConstructor<'_, C> {
    type Refusal = C::Refusal;
    fn contract(&self) -> conduit_core::Kind {
        self.owner.contract()
    }
    fn result_type(&self) -> conduit_core::StructuredInfoType {
        self.owner.result_type()
    }
    fn prepare_configuration(
        &self,
        configuration: &[ConfigurationEntry],
    ) -> Result<conduit_core::StructuredInfoValue, Self::Refusal> {
        self.owner.prepare_configuration(configuration)
    }
}
impl<C: LiteralValueConstructor> LiteralValueConstructor for FamilyContextConstructor<'_, C> {
    fn context_types(&self) -> Vec<(String, conduit_core::StructuredInfoType)> {
        self.fields.clone()
    }
    fn parser_contract(&self) -> &str {
        self.owner.parser_contract()
    }
    fn lexical_policy(&self) -> TypedLiteralLexicalPolicy {
        self.owner.lexical_policy()
    }
    fn literal_configuration(
        &self,
        literal: &TypedGlyphLiteralSyntax,
        context: &[ConfigurationEntry],
    ) -> Result<Vec<ConfigurationEntry>, Self::Refusal> {
        let required = self.owner.context_types();
        let selected = context
            .iter()
            .filter(|entry| required.iter().any(|(key, _)| *key == entry.key))
            .cloned()
            .collect::<Vec<_>>();
        self.owner.literal_configuration(literal, &selected)
    }
}
