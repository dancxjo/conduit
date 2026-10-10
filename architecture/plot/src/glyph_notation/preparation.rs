//! Literal elaboration through an ordinary compiled constructor owner.
use crate::prelude::*;
use crate::{
    prepare_static_constructor, GlyphNotationScope, PreparedStaticValue, ProfileCatalog,
    StartupCatalog, StaticConstructorRefusal, StaticValueConstructor, SyntaxDocument,
    TypedGlyphLiteralSyntax, TypedLiteralLexicalPolicy,
};
use conduit_core::{ConfigurationEntry, ConfigurationValue, SourceDocumentId};
use sha2::{Digest, Sha256};

/// Domain-owned conversion of raw lexical payload plus explicit checked context
/// into its existing ordinary constructor arguments. This is preparation-time
/// Rust composition, never a parser callback supplied by Source or a Host.
pub trait LiteralValueConstructor: StaticValueConstructor {
    /// Exact Types of explicitly selected ordinary Source context values.
    fn context_types(&self) -> Vec<(String, conduit_core::StructuredInfoType)> {
        Vec::new()
    }
    fn parser_contract(&self) -> &str;
    fn lexical_policy(&self) -> TypedLiteralLexicalPolicy;
    fn literal_configuration(
        &self,
        literal: &TypedGlyphLiteralSyntax,
        context: &[ConfigurationEntry],
    ) -> Result<Vec<ConfigurationEntry>, Self::Refusal>;
}

#[derive(Debug)]
pub enum LiteralPreparationRefusal<E> {
    Source,
    SourceContext(crate::SyntaxCheckDiagnostic),
    Scope,
    Identity,
    ContextLimit,
    Owner(E),
    Constructor(StaticConstructorRefusal<E>),
}

/// A successfully executed literal constructor and separate authored custody.
/// This receipt does not claim admission of the surrounding Source program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedGlyphLiteral {
    source_document_id: SourceDocumentId,
    authored: TypedGlyphLiteralSyntax,
    ordinary: PreparedStaticValue,
    pub(crate) source_context: Vec<(crate::Expression, crate::CanonicalStructuredStartupValue)>,
}
impl PreparedGlyphLiteral {
    pub fn source_document_id(&self) -> &SourceDocumentId {
        &self.source_document_id
    }
    pub fn authored(&self) -> &TypedGlyphLiteralSyntax {
        &self.authored
    }
    pub fn ordinary(&self) -> &PreparedStaticValue {
        &self.ordinary
    }
}

impl GlyphNotationScope {
    pub fn prepare_literal<C: LiteralValueConstructor>(
        &self,
        document: &SyntaxDocument,
        literal: &TypedGlyphLiteralSyntax,
        context: &[ConfigurationEntry],
        constructor: &C,
        startup: &StartupCatalog,
        profile: &ProfileCatalog,
    ) -> Result<PreparedGlyphLiteral, LiteralPreparationRefusal<C::Refusal>> {
        use LiteralPreparationRefusal as R;
        if literal.source_document_id != document.source_document_id()
            || !document.diagnostics.is_empty()
            || document.round_trip().len() > crate::MAXIMUM_PLOT_SOURCE_BYTES
        {
            return Err(R::Source);
        }
        let fresh = crate::resolve_glyph_notation_scope(document, startup).map_err(|_| R::Scope)?;
        let binding = self.binding(&literal.alias.text).ok_or(R::Scope)?;
        if fresh.binding(&literal.alias.text) != Some(binding) {
            return Err(R::Scope);
        }
        let source = document.round_trip();
        let authored = source
            .get(literal.authored.span.start..literal.authored.span.end)
            .ok_or(R::Source)?;
        if authored != literal.authored.text {
            return Err(R::Source);
        }
        let scanned = self
            .scan_literal(&literal.alias.text, authored)
            .map_err(|_| R::Identity)?;
        let payload_start = literal.authored.span.start + scanned.payload_bytes.start;
        let payload_end = literal.authored.span.start + scanned.payload_bytes.end;
        let expected_identity: [u8; 32] =
            Sha256::digest(binding.family.identity_bytes().map_err(|_| R::Identity)?).into();
        if scanned.consumed_bytes != authored.len()
            || scanned.branch.delimiter != literal.delimiter
            || expected_identity != literal.family_identity
            || scanned.raw_payload != literal.raw_payload.text
            || scanned.payload != literal.payload
            || scanned.case_insensitive != literal.case_insensitive
            || scanned.anchored_start != literal.anchored_start
            || scanned.anchored_end != literal.anchored_end
            || Some(literal.raw_payload.span)
                != crate::source_span(source, payload_start..payload_end)
            || Some(literal.alias.span)
                != crate::source_span(
                    source,
                    literal.authored.span.start
                        ..literal.authored.span.start + literal.alias.text.len(),
                )
            || Some(literal.authored.span)
                != crate::source_span(
                    source,
                    literal.authored.span.start..literal.authored.span.end,
                )
        {
            return Err(R::Identity);
        }
        let contract = constructor.contract();
        if scanned.branch.parser_contract != constructor.parser_contract()
            || scanned.branch.lexical_policy != constructor.lexical_policy()
            || scanned.branch.constructor_kind != contract.kind_id
            || scanned.branch.constructor_revision != contract.kind_contract_revision
            || scanned.branch.result_type != constructor.result_type()
        {
            return Err(R::Identity);
        }
        // Admit context work/storage before any owner recipe may clone fields.
        if context.len() > conduit_core::MAXIMUM_STRUCTURED_RECORD_FIELDS {
            return Err(R::ContextLimit);
        }
        let mut context_bytes = 0usize;
        for entry in context {
            let value_bytes = match &entry.value {
                ConfigurationValue::Text(value) => value.len(),
                ConfigurationValue::Structured(value) => value.canonical_value().len(),
                _ => core::mem::size_of::<ConfigurationValue>(),
            };
            context_bytes = context_bytes
                .saturating_add(entry.key.len())
                .saturating_add(value_bytes);
            if context_bytes > 1024 * 1024 {
                return Err(R::ContextLimit);
            }
        }
        let configuration = constructor
            .literal_configuration(literal, context)
            .map_err(R::Owner)?;
        let ordinary = prepare_static_constructor(constructor, startup, profile, &configuration)
            .map_err(R::Constructor)?;
        Ok(PreparedGlyphLiteral {
            source_document_id: document.source_document_id(),
            authored: literal.clone(),
            ordinary,
            source_context: Vec::new(),
        })
    }
}
