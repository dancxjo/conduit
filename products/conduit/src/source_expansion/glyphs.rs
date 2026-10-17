//! Authored glyphs correlated with sealed ordinary constructor results.
use super::SourceSpan;
use conduit_plot::{
    BackStatement, CanonicalStructuredStartupValue, CheckedSyntaxDocument, StartupCatalog,
    SyntaxDocument,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct GlyphView<'a> {
    authored: &'a str,
    source_span: SourceSpan,
    raw_payload: &'a str,
    payload_span: SourceSpan,
    alias: &'a str,
    import_path: &'a str,
    family_revision: &'a str,
    family_identity: String,
    notation_source_id: &'a str,
    package_content_digest: String,
    delimiters: String,
    parser_contract: &'a str,
    lexical_policy: String,
    ordinary_constructor: &'a str,
    constructor_revision: &'a str,
    checked_value: ValueView,
    basis: Vec<BasisView<'a>>,
}

pub(super) struct GlyphViews<'a> {
    pub(super) glyphs: Vec<GlyphView<'a>>,
    pub(super) contexts: Vec<ContextView<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct ContextView<'a> {
    authored: &'a str,
    source_span: SourceSpan,
    checked_value: ValueView,
}

#[derive(Debug, Serialize)]
struct BasisView<'a> {
    key: &'a str,
    local: &'a str,
    source_span: SourceSpan,
    context_index: usize,
    checked_value_digest: String,
}

#[derive(Debug, Serialize)]
struct ValueView {
    result_type: String,
    type_digest: String,
    value_digest: String,
    /// The existing Core structured-value codec carries the exact Type too.
    canonical_value_hex: String,
}

fn value_view(value: &CanonicalStructuredStartupValue) -> Result<ValueView, String> {
    let concrete = value
        .try_concrete()
        .ok_or("admitted glyph inspection requires concrete Info")?;
    let refusal = |error| format!("invalid admitted glyph inspection value: {error:?}");
    Ok(ValueView {
        result_type: value
            .value_type()
            .profile()
            .map_err(refusal)?
            .value_kind()
            .as_str()
            .into(),
        type_digest: hex(&value.value_type().semantic_digest().map_err(refusal)?),
        value_digest: hex(&concrete.semantic_digest().map_err(refusal)?),
        canonical_value_hex: hex(&concrete.canonical_bytes().map_err(refusal)?),
    })
}

pub(super) fn views<'a>(
    document: &'a SyntaxDocument,
    checked: &'a CheckedSyntaxDocument,
    startup: &'a StartupCatalog,
) -> Result<GlyphViews<'a>, String> {
    if checked.source_document_id != document.source_document_id() {
        return Err("glyph inspection requires the exact checked Source identity".into());
    }
    let mut contexts = Vec::new();
    let mut context_indices = std::collections::BTreeMap::new();
    for (expression, value) in checked.glyph_values.contexts() {
        context_indices.insert((expression.span.start, expression.span.end), contexts.len());
        contexts.push(ContextView {
            authored: &expression.text,
            source_span: expression.span.into(),
            checked_value: value_view(value)?,
        });
    }
    let mut result = Vec::new();
    for (literal, value) in checked.glyph_values.literals() {
        let import = document
            .uses
            .iter()
            .find(|import| import.alias.text == literal.alias.text)
            .ok_or("admitted glyph has no authored import")?;
        let family = startup
            .typed_literal_family(&import.path)
            .ok_or("admitted glyph family is absent")?;
        let branch = family
            .branch(literal.delimiter)
            .ok_or("admitted glyph branch is absent")?;
        let plot = document.plots.iter().find(|plot| {
            literal.authored.span.start >= plot.span.start
                && literal.authored.span.end <= plot.span.end
        });
        let mut basis = Vec::new();
        for (key, local) in &import.glyph_context {
            let plot = plot.ok_or("contextual glyph has no containing Plot")?;
            let selected = plot
                .back
                .iter()
                .find_map(|statement| match statement {
                    BackStatement::LocalValue(value) if value.name.text == local.text => {
                        Some(value)
                    }
                    _ => None,
                })
                .ok_or("selected glyph context local is absent")?;
            let (expression, _) = checked
                .glyph_values
                .context_value(selected.value.span)
                .ok_or("selected glyph context lacks sealed admission")?;
            if expression != &selected.value {
                return Err("selected glyph context differs from its sealed Source node".into());
            }
            basis.push(BasisView {
                key: &key.text,
                local: &local.text,
                source_span: expression.span.into(),
                context_index: context_indices[&(expression.span.start, expression.span.end)],
                checked_value_digest: contexts
                    [context_indices[&(expression.span.start, expression.span.end)]]
                    .checked_value
                    .value_digest
                    .clone(),
            });
        }
        let (open, close) = literal.delimiter.pair();
        result.push(GlyphView {
            authored: &literal.authored.text,
            source_span: literal.authored.span.into(),
            raw_payload: &literal.raw_payload.text,
            payload_span: literal.raw_payload.span.into(),
            alias: &literal.alias.text,
            import_path: &import.path,
            family_revision: &family.revision,
            family_identity: hex(&literal.family_identity),
            notation_source_id: family.origin.source_document_id.as_str(),
            package_content_digest: hex(&family.origin.package_content_digest),
            delimiters: format!("{open}{close}"),
            parser_contract: &branch.parser_contract,
            lexical_policy: format!("{:?}", branch.lexical_policy),
            ordinary_constructor: branch.constructor_kind.as_str(),
            constructor_revision: branch.constructor_revision.as_str(),
            checked_value: value_view(value)?,
            basis,
        });
    }
    Ok(GlyphViews {
        glyphs: result,
        contexts,
    })
}

pub(super) fn render(glyphs: &[GlyphView<'_>], output: &mut String) {
    for glyph in glyphs {
        output.push_str(&format!(
            "\n{}:{} `{}`\n  glyph family: {} @ {}\n  parser: {} ({})\n  ordinary constructor: {} @ {}\n  checked Type: {}\n  checked value digest: {}\n  canonical Core value: {}\n",
            glyph.source_span.line, glyph.source_span.column, glyph.authored,
            glyph.import_path, glyph.family_revision, glyph.parser_contract, glyph.lexical_policy,
            glyph.ordinary_constructor, glyph.constructor_revision, glyph.checked_value.result_type,
            glyph.checked_value.value_digest, glyph.checked_value.canonical_value_hex,
        ));
        for basis in &glyph.basis {
            output.push_str(&format!(
                "  basis {} = {} at {}:{} (checked value {})\n",
                basis.key,
                basis.local,
                basis.source_span.line,
                basis.source_span.column,
                basis.checked_value_digest
            ));
        }
    }
}

pub(super) fn render_contexts(contexts: &[ContextView<'_>], output: &mut String) {
    for (index, context) in contexts.iter().enumerate() {
        output.push_str(&format!(
            "\nShared glyph context {index} at {}:{}\n  authored: {}\n  checked Type: {}\n  checked value digest: {}\n  canonical Core value: {}\n",
            context.source_span.line, context.source_span.column, context.authored,
            context.checked_value.result_type, context.checked_value.value_digest,
            context.checked_value.canonical_value_hex,
        ));
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut text, "{byte:02x}").expect("String writes are infallible");
    }
    text
}
