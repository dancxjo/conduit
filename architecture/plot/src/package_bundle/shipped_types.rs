//! Transactional checked Type export installation.
use super::PackageExportCatalog;
use crate::prelude::*;

impl PackageExportCatalog {
    /// Installs shipped Type paths for downstream `with ... as ...` checking.
    /// The package path is a source lookup name only; the checked Type keeps
    /// the same semantic identity it had in its defining pack.
    pub fn install_shipped_types(
        &self,
        catalog: &mut crate::StartupCatalog,
    ) -> Result<Vec<crate::CheckedNativeType>, crate::SyntaxCheckDiagnostic> {
        let mut aliased = catalog.clone();
        for document in &self.type_documents {
            aliased = crate::native_type::install_import_aliases(document, &aliased)?;
        }
        let (checked, owner) =
            crate::native_type::check_native_types(&self.type_definitions, &aliased)?;
        let mut staged = catalog.clone();
        let mut shipped = Vec::new();
        for (source_path, syntax) in &self.type_exports {
            if !syntax.parameters.is_empty() {
                crate::native_type::family::install(
                    &mut staged,
                    source_path,
                    syntax,
                    &self.type_definitions,
                    &owner,
                    self.package_content_digest,
                    &self.type_origins,
                )?;
                continue;
            }
            let value_type = checked
                .iter()
                .find(|candidate| candidate.name == syntax.name.text)
                .ok_or_else(|| crate::SyntaxCheckDiagnostic {
                    code: "CND-FRM-058",
                    span: syntax.name.span,
                    message: "shipped Type families require checked family installation".into(),
                })?;
            staged
                .insert_native_type(
                    source_path.clone(),
                    value_type.value_type.clone(),
                    value_type.value_contracts.clone(),
                    value_type.invariants.clone(),
                )
                .map_err(|message| crate::SyntaxCheckDiagnostic {
                    code: "CND-FRM-058",
                    span: syntax.name.span,
                    message,
                })?;
            crate::native_type::family::source::register(
                &mut staged,
                source_path,
                &self.type_origins[&syntax.name.text],
                syntax.name.span,
            )?;
            shipped.push(value_type.clone());
        }
        *catalog = staged;
        Ok(shipped)
    }
}
