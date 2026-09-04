use conduit_form::{
    source_document_identity, DiagnosticSeverity, StructuredDiagnosticV1, SyntaxCheckDiagnostic,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) fn run(path: &Path, json: bool) -> Result<bool, String> {
    let document = crate::form_source::load(path)?;
    let diagnostics = if document.syntax.diagnostics.is_empty() {
        conduit_form::check_syntax_document(&document.syntax, &document.startup)
            .err()
            .map(|diagnostic| structured(&document.source, &diagnostic))
            .transpose()?
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        document
            .syntax
            .diagnostics
            .iter()
            .map(|diagnostic| {
                structured_parts(
                    &document.source,
                    diagnostic.code,
                    &diagnostic.message,
                    diagnostic.span,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&diagnostics).map_err(|error| error.to_string())?
        );
    } else if diagnostics.is_empty() {
        println!("No form diagnostics.");
    } else {
        for diagnostic in &diagnostics {
            println!("{}", diagnostic.render_human());
        }
    }
    Ok(diagnostics.is_empty())
}

fn structured(
    source: &str,
    diagnostic: &SyntaxCheckDiagnostic,
) -> Result<StructuredDiagnosticV1, String> {
    structured_parts(
        source,
        diagnostic.code,
        &diagnostic.message,
        diagnostic.span,
    )
}

fn structured_parts(
    source: &str,
    code: &'static str,
    message: &str,
    span: conduit_form::Span,
) -> Result<StructuredDiagnosticV1, String> {
    StructuredDiagnosticV1::new(
        code,
        DiagnosticSeverity::Error,
        message,
        source_document_identity(source),
        Some(conduit_form::source_document_identity(source)),
        Some(span.into()),
        Vec::new(),
        BTreeMap::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(str::to_owned)
}
