use conduit_plot::{
    source_document_identity, DiagnosticSeverity, StructuredDiagnosticV1, SyntaxCheckDiagnostic,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) fn run(path: &Path, json: bool) -> Result<bool, String> {
    let document = crate::plot_source::load(path)?;
    let diagnostics = if document.syntax.diagnostics.is_empty() {
        let diagnostic =
            match conduit_plot::check_syntax_document(&document.syntax, &document.startup) {
                Err(diagnostic) => Some(structured(&document.source, &diagnostic)?),
                Ok(checked) => {
                    let ipa = conduit_speech::ipa_constructors::validate_source(
                        &document.syntax,
                        &checked,
                    )
                    .err()
                    .map(|diagnostic| {
                        structured_parts(
                            &document.source,
                            "CND-SPC-IPA",
                            &format!("{:?}", diagnostic.cause.refusal),
                            diagnostic.span,
                        )
                    })
                    .transpose()?;
                    if ipa.is_some() {
                        ipa
                    } else {
                        conduit_plot::quantity_conversion::validate_source(
                            &document.syntax,
                            &checked,
                        )
                        .err()
                        .map(|diagnostic| {
                            structured_parts(
                                &document.source,
                                "CND-QTY-001",
                                &format!("{:?}", diagnostic.refusal),
                                diagnostic.span,
                            )
                        })
                        .transpose()?
                    }
                }
            };
        diagnostic.into_iter().collect::<Vec<_>>()
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
        println!("No plot diagnostics.");
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
    span: conduit_plot::Span,
) -> Result<StructuredDiagnosticV1, String> {
    StructuredDiagnosticV1::new(
        code,
        DiagnosticSeverity::Error,
        message,
        source_document_identity(source),
        Some(conduit_plot::source_document_identity(source)),
        Some(span.into()),
        Vec::new(),
        BTreeMap::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(str::to_owned)
}
