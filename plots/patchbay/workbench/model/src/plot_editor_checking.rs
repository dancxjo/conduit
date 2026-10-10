//! One scoped parsing and checked-constructor entrance for editor revisions.
use super::{check_error_revision, graph_revision, CheckedRevision, PlotEditorError};
use conduit_plot::{
    check_syntax_document_with_literal_constructors, parse_syntax_document_with_glyph_notations,
    CheckedSyntaxDocument, ProfileCatalog, StartupCatalog, SyntaxCheckDiagnostic, SyntaxDocument,
};

pub(super) fn checked_source(
    source: &str,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<(SyntaxDocument, CheckedSyntaxDocument), SyntaxCheckDiagnostic> {
    let syntax = parse_syntax_document_with_glyph_notations(source, startup);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(SyntaxCheckDiagnostic {
            code: diagnostic.code,
            message: diagnostic.message.clone(),
            span: diagnostic.span,
        });
    }
    let checked = check_syntax_document_with_literal_constructors(&syntax, startup, profile)?;
    Ok((syntax, checked))
}

pub(crate) fn check_revision_with_catalog(
    revision: u64,
    source: &str,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<CheckedRevision, PlotEditorError> {
    match checked_source(source, startup, profile) {
        Ok((syntax, checked)) => graph_revision(revision, &syntax.plots, checked),
        Err(diagnostic) => Ok(check_error_revision(revision, diagnostic)),
    }
}
