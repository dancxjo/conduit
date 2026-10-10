//! Document-scoped consumption of sealed ordinary constructor receipts.
use crate::*;
use alloc::collections::BTreeMap;
use sha2::{Digest, Sha256};

/// Checks ordinary Source using successfully prepared domain constructor results.
/// Receipts are bound to exact parsed literal nodes, never substring matches or
/// expected-Type dispatch. The supplied startup catalog remains unchanged.
pub fn admit_glyph_values(
    document: &SyntaxDocument,
    catalog: &StartupCatalog,
    receipts: &[PreparedGlyphLiteral],
) -> Result<AdmittedGlyphValues, SyntaxCheckDiagnostic> {
    let fail = |span, message: &str| SyntaxCheckDiagnostic {
        code: "CND-GLY-001",
        span,
        message: message.into(),
    };
    let start = Span {
        start: 0,
        end: 0,
        line: 1,
        column: 1,
        end_line: 1,
        end_column: 1,
    };
    if receipts.len() > 64 || document.round_trip().len() > MAXIMUM_PLOT_SOURCE_BYTES {
        return Err(fail(
            start,
            "prepared glyph admission exceeds its finite Source or receipt bound",
        ));
    }
    let fresh = parse_syntax_document_with_glyph_notations(document.round_trip(), catalog);
    if fresh != *document || !fresh.diagnostics.is_empty() {
        return Err(fail(
            start,
            "prepared glyph admission requires the exact scoped Source AST",
        ));
    }
    let scope = resolve_glyph_notation_scope(document, catalog)?;
    let nodes = document_literals(document).map_err(|span| {
        fail(
            span,
            "glyph startup traversal exceeds its finite node bound",
        )
    })?;
    let mut admitted = BTreeMap::new();
    let mut retained_bytes = 0usize;
    for receipt in receipts {
        let literal = receipt.authored();
        let span = literal.authored.span;
        let key = (span.start, span.end);
        if receipt.source_document_id() != &document.source_document_id()
            || nodes.get(&key).copied() != Some(literal)
            || admitted.contains_key(&key)
        {
            return Err(fail(
                span,
                "foreign, duplicate or non-node prepared glyph receipt",
            ));
        }
        let binding = scope
            .binding(&literal.alias.text)
            .ok_or_else(|| fail(span, "prepared glyph prefix is outside this Source scope"))?;
        let identity = binding
            .family
            .identity_bytes()
            .map_err(|_| fail(span, "invalid glyph family identity"))?;
        let branch = binding
            .family
            .branch(literal.delimiter)
            .ok_or_else(|| fail(span, "prepared glyph delimiter is undeclared"))?;
        let ordinary = receipt.ordinary();
        if literal.family_identity != <[u8; 32]>::from(Sha256::digest(identity))
            || ordinary.constructor_kind() != &branch.constructor_kind
            || ordinary.constructor_revision() != &branch.constructor_revision
            || ordinary.value().value_type() != &branch.result_type
        {
            return Err(fail(
                span,
                "prepared glyph constructor or family identity is stale",
            ));
        }
        let bytes = ordinary
            .value()
            .canonical_bytes()
            .map_err(|_| fail(span, "prepared glyph result exceeds canonical bounds"))?;
        retained_bytes = retained_bytes
            .saturating_add(bytes.len())
            .saturating_add(literal.authored.text.len())
            .saturating_add(literal.raw_payload.text.len())
            .saturating_add(literal.payload.len());
        if retained_bytes > 1024 * 1024 {
            return Err(fail(
                span,
                "combined prepared glyph values exceed the finite byte bound",
            ));
        }
        admitted.insert(
            key,
            (
                literal.clone(),
                CanonicalStructuredStartupValue::from_checked_value(ordinary.value()),
            ),
        );
    }
    for (key, literal) in nodes {
        if !admitted.contains_key(&key) {
            return Err(fail(
                literal.authored.span,
                "typed glyph startup value lacks ordinary constructor admission",
            ));
        }
    }
    Ok(AdmittedGlyphValues { values: admitted })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdmittedGlyphValues {
    pub(crate) values:
        BTreeMap<(usize, usize), (TypedGlyphLiteralSyntax, CanonicalStructuredStartupValue)>,
}
impl AdmittedGlyphValues {
    pub(crate) fn for_plot(catalog: &StartupCatalog, span: Span) -> Self {
        Self {
            values: catalog
                .prepared_glyph_values
                .iter()
                .filter(|((start, end), _)| *start >= span.start && *end <= span.end)
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
        }
    }
    pub(crate) fn bind_identity(
        &self,
        base: conduit_core::CheckedPlotId,
    ) -> conduit_core::CheckedPlotId {
        if self.values.is_empty() {
            return base;
        }
        let mut text = format!("checked-glyph-values@1:{}", base.as_str());
        for ((start, end), (literal, value)) in &self.values {
            text.push_str(&format!(
                "|{start}:{end}:{}:{}",
                literal.source_document_id.as_str(),
                value.canonical_identity()
            ));
        }
        conduit_core::CheckedPlotId::from(crate::hash_string(&text))
    }

    pub(crate) fn resolve(
        &self,
        literal: &TypedGlyphLiteralSyntax,
    ) -> Option<&CanonicalStructuredStartupValue> {
        let span = literal.authored.span;
        let (authored, value) = self.values.get(&(span.start, span.end))?;
        (authored == literal).then_some(value)
    }
}

pub fn check_syntax_document_with_prepared_glyph_literals(
    document: &SyntaxDocument,
    catalog: &StartupCatalog,
    receipts: &[PreparedGlyphLiteral],
) -> Result<CheckedSyntaxDocument, SyntaxCheckDiagnostic> {
    let admitted = admit_glyph_values(document, catalog, receipts)?;
    let mut scoped = catalog.clone();
    scoped.prepared_glyph_values = admitted.values;
    check_syntax_document(document, &scoped)
}

// Collect exact authored nodes; execution remains behind expression admission.
fn document_literals(
    document: &SyntaxDocument,
) -> Result<BTreeMap<(usize, usize), &TypedGlyphLiteralSyntax>, Span> {
    let mut roots = Vec::new();
    let mut plots: Vec<_> = document.plots.iter().collect();
    let mut visited = 0usize;
    while let Some(plot) = plots.pop() {
        visited += 1;
        if visited > 4096 {
            return Err(plot.span);
        }
        plots.extend(&plot.local_plots);
        for parameter in &plot.front.startup_parameters {
            if let Some(value) = &parameter.default {
                roots.push(&value.syntax);
            }
        }
        for statement in &plot.back {
            match statement {
                BackStatement::LocalValue(local) => roots.push(&local.value.syntax),
                BackStatement::NamedGear(gear) => {
                    arguments(&gear.invocation, &mut roots);
                    if let Some(value) = gear
                        .retained
                        .as_ref()
                        .and_then(|retained| retained.initial.as_ref())
                    {
                        roots.push(&value.syntax);
                    }
                }
                BackStatement::Cord(cord) => stages(&cord.stages, &mut roots),
                BackStatement::MatchedRoute(route) => {
                    for arm in &route.arms {
                        stages(&arm.stages, &mut roots);
                    }
                }
                BackStatement::Pool(_) => {}
            }
            if roots.len() + plots.len() > 4096 {
                return Err(plot.span);
            }
        }
    }
    let mut found = BTreeMap::new();
    while let Some(node) = roots.pop() {
        visited += 1;
        if visited > 4096 {
            return Err(node.span());
        }
        match node {
            ExpressionSyntax::TypedGlyphLiteral(literal) => {
                let span = literal.authored.span;
                if found
                    .insert((span.start, span.end), literal.as_ref())
                    .is_some()
                {
                    return Err(span);
                }
            }
            ExpressionSyntax::Collection { values, .. }
            | ExpressionSyntax::Tuple { values, .. }
            | ExpressionSyntax::SemanticCall {
                arguments: values, ..
            } => roots.extend(values),
            ExpressionSyntax::Record { fields, .. } => {
                roots.extend(fields.iter().map(|field| &field.value))
            }
            ExpressionSyntax::Variant { payload, .. }
            | ExpressionSyntax::Unary {
                operand: payload, ..
            }
            | ExpressionSyntax::Projection { value: payload, .. } => roots.push(payload),
            ExpressionSyntax::Binary { left, right, .. } => {
                roots.push(left);
                roots.push(right);
            }
            ExpressionSyntax::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => {
                roots.extend([condition.as_ref(), when_true.as_ref(), when_false.as_ref()]);
            }
            ExpressionSyntax::Atomic(_) | ExpressionSyntax::Input(_) => {}
        }
        if roots.len() > 4096 {
            return Err(node.span());
        }
    }
    Ok(found)
}
fn arguments<'a>(invocation: &'a Invocation, roots: &mut Vec<&'a ExpressionSyntax>) {
    for argument in &invocation.arguments {
        roots.push(
            &match argument {
                Argument::Positional(value) | Argument::Named { value, .. } => value,
            }
            .syntax,
        );
    }
}
fn stages<'a>(stages: &'a [CordStage], roots: &mut Vec<&'a ExpressionSyntax>) {
    for stage in stages {
        match stage {
            CordStage::InlineGear(invocation) | CordStage::RelationalGear { invocation, .. } => {
                arguments(invocation, roots)
            }
            CordStage::PureExpression(value)
            | CordStage::Literal(value)
            | CordStage::When(value) => roots.push(&value.syntax),
            _ => {}
        }
    }
}
