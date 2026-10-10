//! Compiled semantic owners composed into checking; Source installs no code.
use crate::*;
use core::fmt;

trait CompiledOwner: Send + Sync {
    fn text_constraint(
        &self,
        value: &conduit_core::StructuredInfoValue,
        bound: u32,
        negated: bool,
    ) -> Result<Option<conduit_core::ValueConstraint>, String>;
    fn context_types(&self) -> Vec<(String, conduit_core::StructuredInfoType)>;
    fn contract(&self) -> conduit_core::Kind;
    fn prepare(
        &self,
        scope: &GlyphNotationScope,
        document: &SyntaxDocument,
        plot: &str,
        literal: &TypedGlyphLiteralSyntax,
        startup: &StartupCatalog,
        profile: &ProfileCatalog,
    ) -> Result<PreparedGlyphLiteral, SyntaxCheckDiagnostic>;
}
impl<C> CompiledOwner for C
where
    C: LiteralValueConstructor + Send + Sync,
    C::Refusal: fmt::Debug,
{
    fn text_constraint(
        &self,
        value: &conduit_core::StructuredInfoValue,
        bound: u32,
        negated: bool,
    ) -> Result<Option<conduit_core::ValueConstraint>, String> {
        LiteralValueConstructor::text_constraint(self, value, bound, negated)
            .map_err(|error| format!("{error:?}"))
    }
    fn context_types(&self) -> Vec<(String, conduit_core::StructuredInfoType)> {
        LiteralValueConstructor::context_types(self)
    }
    fn contract(&self) -> conduit_core::Kind {
        StaticValueConstructor::contract(self)
    }
    fn prepare(
        &self,
        scope: &GlyphNotationScope,
        document: &SyntaxDocument,
        plot: &str,
        literal: &TypedGlyphLiteralSyntax,
        startup: &StartupCatalog,
        profile: &ProfileCatalog,
    ) -> Result<PreparedGlyphLiteral, SyntaxCheckDiagnostic> {
        scope
            .prepare_literal_from_authored_context(document, plot, literal, self, startup, profile)
            .map_err(|error| match error {
                LiteralPreparationRefusal::SourceContext(diagnostic) => diagnostic,
                error => SyntaxCheckDiagnostic {
                    code: "CND-GLY-003",
                    span: literal.authored.span,
                    message: format!("compiled glyph constructor refused: {error:?}"),
                },
            })
    }
}

/// References only immutable Rust owners installed by semantic composition.
/// Equality preserves the actual owner object, including when catalogs clone.
#[derive(Clone, Copy)]
pub(crate) struct InstalledLiteralOwner(&'static dyn CompiledOwner);
impl InstalledLiteralOwner {
    pub(super) fn context_types(&self) -> Vec<(String, conduit_core::StructuredInfoType)> {
        self.0.context_types()
    }
}
impl fmt::Debug for InstalledLiteralOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InstalledLiteralOwner")
            .field(&self.0.contract().kind_id)
            .finish()
    }
}
impl PartialEq for InstalledLiteralOwner {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0)
    }
}
impl Eq for InstalledLiteralOwner {}

impl StartupCatalog {
    pub(crate) fn glyph_text_constraint(
        &self,
        literal: &TypedGlyphLiteralSyntax,
        bound: u32,
        negated: bool,
    ) -> Result<conduit_core::ValueConstraint, String> {
        use sha2::{Digest, Sha256};
        let (authored, prepared) = self
            .prepared_glyph_values
            .get(&(literal.authored.span.start, literal.authored.span.end))
            .ok_or("glyph pattern requires ordinary constructor admission")?;
        if authored != literal {
            return Err("glyph pattern has foreign Source custody".into());
        }
        let family = self
            .installed_literal_families()
            .find_map(|(_, family)| {
                let identity: [u8; 32] = Sha256::digest(family.identity_bytes().ok()?).into();
                (identity == literal.family_identity).then_some(family)
            })
            .ok_or("glyph pattern family is missing")?;
        let branch = family
            .branch(literal.delimiter)
            .ok_or("glyph pattern branch is missing")?;
        let owner = self
            .literal_owners
            .get(branch.constructor_kind.as_str())
            .ok_or("glyph pattern has no compiled consumer")?;
        let value = prepared
            .try_concrete()
            .ok_or("glyph pattern value is not concrete")?;
        owner
            .0
            .text_constraint(&value, bound, negated)?
            .ok_or("glyph value has no finite Text constraint consumer".into())
    }

    /// Attach a compiled ordinary constructor to its already checked shipped
    /// branches. Hosts and authored metadata cannot populate this table.
    pub fn install_literal_constructor<C>(
        &mut self,
        constructor: &'static C,
        profile: &ProfileCatalog,
    ) -> Result<(), String>
    where
        C: LiteralValueConstructor + Send + Sync,
        C::Refusal: fmt::Debug,
    {
        let contract = constructor.contract();
        let kind = contract.kind_id.as_str();
        if self.literal_owners.len()
            >= MAXIMUM_TYPED_LITERAL_FAMILIES * MAXIMUM_TYPED_LITERAL_BRANCHES
            || self.literal_owners.contains_key(kind)
            || profile.canonical_kind(&contract.kind_id) != Some(&contract)
            || self.signature(kind).is_none()
        {
            return Err(
                "compiled glyph owner is duplicate, uninstalled or exceeds its bound".into(),
            );
        }
        let mut found = false;
        for (_, family) in self.installed_literal_families() {
            for branch in &family.branches {
                if branch.constructor_kind != contract.kind_id {
                    continue;
                }
                found = true;
                if branch.constructor_revision != contract.kind_contract_revision
                    || branch.result_type != constructor.result_type()
                    || branch.parser_contract != constructor.parser_contract()
                    || branch.lexical_policy != constructor.lexical_policy()
                {
                    return Err(
                        "compiled glyph owner differs from its checked shipped branch".into(),
                    );
                }
            }
        }
        if !found {
            return Err("compiled glyph owner has no checked shipped branch".into());
        }
        self.literal_owners
            .insert(kind.into(), InstalledLiteralOwner(constructor));
        Ok(())
    }
}

/// The common Source checking entrance for installed semantic composition.
/// Resolve owners only by declared branch Kind, never by inferred result Type.
pub fn check_syntax_document_with_literal_constructors(
    document: &SyntaxDocument,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<CheckedSyntaxDocument, SyntaxCheckDiagnostic> {
    let scope = resolve_glyph_notation_scope(document, startup)?;
    let fail = |span, message: &str| SyntaxCheckDiagnostic {
        code: "CND-GLY-003",
        span,
        message: message.into(),
    };
    let nodes = super::admission::document_literals(document)
        .map_err(|span| fail(span, "glyph traversal exceeds its finite node bound"))?;
    if nodes.is_empty() {
        return check_syntax_document(document, startup);
    }
    let first = nodes.values().next().unwrap().authored.span;
    if nodes.len() > 64 {
        return Err(fail(first, "glyph preparation exceeds 64 literal receipts"));
    }
    let mut receipts = Vec::with_capacity(nodes.len());
    for literal in nodes.values() {
        let span = literal.authored.span;
        let binding = scope
            .binding(&literal.alias.text)
            .ok_or_else(|| fail(span, "glyph prefix has no checked import"))?;
        let branch = binding
            .family
            .branch(literal.delimiter)
            .ok_or_else(|| fail(span, "glyph delimiter has no checked branch"))?;
        let owner = startup
            .literal_owners
            .get(branch.constructor_kind.as_str())
            .ok_or_else(|| fail(span, "glyph branch has no installed compiled constructor"))?;
        let mut plots = document
            .plots
            .iter()
            .filter(|plot| span.start >= plot.span.start && span.end <= plot.span.end);
        let plot = plots
            .next()
            .ok_or_else(|| fail(span, "glyph literal has no containing Plot"))?;
        if plots.next().is_some() {
            return Err(fail(span, "glyph literal has ambiguous Plot custody"));
        }
        receipts.push(owner.0.prepare(
            &scope,
            document,
            &plot.name.text,
            literal,
            startup,
            profile,
        )?);
    }
    check_syntax_document_with_prepared_glyph_literals(document, startup, &receipts)
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn source_cannot_supply_a_missing_compiled_owner() {
        let (mut startup, profile, family) = crate::glyph_notation_test_support::fixture();
        startup
            .insert_typed_literal_family("fixture/notation", family, &profile)
            .unwrap();
        let document = parse_syntax_document_with_glyph_notations(
            "with fixture/notation as r\nplot example {\n value = r/a/\n}\n",
            &startup,
        );
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        let error = check_syntax_document_with_literal_constructors(&document, &startup, &profile)
            .unwrap_err();
        assert!(
            error.message.contains("no installed compiled constructor"),
            "{error:?}"
        );
    }
}
