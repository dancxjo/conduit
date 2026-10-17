//! Type-only preparation for generic Back contracts before full plot checking.
use crate::{StartupCatalog, SyntaxCheckDiagnostic, SyntaxDocument};

/// Checked schemas with Source names retained only for preparation lookup.
/// Executable contracts still use canonical value Kind identity.
pub struct PreparedSourceTypes {
    catalog: StartupCatalog,
}
impl PreparedSourceTypes {
    pub fn named_type(&self, name: &str) -> Option<&conduit_core::StructuredInfoType> {
        self.catalog.structured_type(name)
    }
}

/// Check declarations and imported native aliases without resolving callable
/// plots. Callers must subsequently check the complete Source against the Backs
/// specialized from these schemas; this result is neither a checked plot nor an
/// executable artifact.
pub fn prepare_source_types(
    document: &SyntaxDocument,
    catalog: &StartupCatalog,
) -> Result<PreparedSourceTypes, SyntaxCheckDiagnostic> {
    if let Some(diagnostic) = document.diagnostics.first() {
        return Err(SyntaxCheckDiagnostic {
            code: diagnostic.code,
            span: diagnostic.span,
            message: diagnostic.message.clone(),
        });
    }
    let aliased = crate::native_type::install_import_aliases(document, catalog)?;
    let (_, catalog) = crate::native_type::check_native_types(&document.types, &aliased)?;
    Ok(PreparedSourceTypes { catalog })
}
