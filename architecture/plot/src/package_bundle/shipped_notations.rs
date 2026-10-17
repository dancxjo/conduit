//! Transactional installation of declared, source-owned glyph families.
use super::PackageExportCatalog;
use crate::prelude::*;
use crate::SyntaxCheckDiagnostic;

impl PackageExportCatalog {
    /// Resolve declared metadata against installed ordinary constructor and Type
    /// truth. This installs no payload parser, callback, offer or executable Gear.
    pub fn install_shipped_glyph_notations(
        &self,
        startup: &mut crate::StartupCatalog,
        profile: &crate::ProfileCatalog,
    ) -> Result<Vec<String>, SyntaxCheckDiagnostic> {
        let mut staged = startup.clone();
        let mut shipped = Vec::new();
        for (path, (syntax, origin)) in &self.glyph_notation_exports {
            let family = crate::glyph_notation::checked_family(
                &syntax.metadata.syntax,
                origin.clone(),
                &staged,
            )?;
            staged
                .insert_typed_literal_family(path.clone(), family, profile)
                .map_err(|message| {
                    crate::glyph_notation::failure(&syntax.metadata.syntax, message)
                })?;
            shipped.push(path.clone());
        }
        *startup = staged;
        Ok(shipped)
    }
}

#[cfg(test)]
mod tests;
