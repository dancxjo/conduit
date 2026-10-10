//! Checked owner module provenance, independent of private preparation names.
use crate::prelude::*;
use alloc::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTypeSourceOrigin {
    pub package_content_digest: [u8; 32],
    pub module_path: String,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub declaration_name: String,
    pub declaration_span: crate::Span,
}

pub(crate) fn from_document(
    document: &crate::SyntaxDocument,
    module_path: &str,
    package_content_digest: [u8; 32],
) -> BTreeMap<String, NativeTypeSourceOrigin> {
    let source_document_id = document.source_document_id();
    document
        .types
        .iter()
        .map(|declaration| {
            (
                declaration.name.text.clone(),
                NativeTypeSourceOrigin {
                    package_content_digest,
                    module_path: module_path.into(),
                    source_document_id: source_document_id.clone(),
                    declaration_name: declaration.name.text.clone(),
                    declaration_span: declaration.span,
                },
            )
        })
        .collect()
}

pub(crate) fn register(
    catalog: &mut crate::StartupCatalog,
    path: &str,
    origin: &NativeTypeSourceOrigin,
    span: crate::Span,
) -> Result<(), crate::SyntaxCheckDiagnostic> {
    if let Some(existing) = catalog.native_type_sources.get(path) {
        return if existing == origin {
            Ok(())
        } else {
            Err(super::super::diagnostic(
                span,
                "conflicting checked Type source provenance".into(),
            ))
        };
    }
    let bytes = |name: &str, origin: &NativeTypeSourceOrigin| {
        name.len()
            .saturating_add(origin.module_path.len())
            .saturating_add(origin.declaration_name.len())
            .saturating_add(origin.source_document_id.as_str().len())
            .saturating_add(32 + 48)
    };
    let total = catalog
        .native_type_sources
        .iter()
        .fold(bytes(path, origin), |total, (name, origin)| {
            total.saturating_add(bytes(name, origin))
        });
    if catalog.native_type_sources.len() >= 4096 || total > 1024 * 1024 {
        return Err(super::super::diagnostic(
            span,
            "checked Type source registry exceeds its finite provenance profile".into(),
        ));
    }
    catalog
        .native_type_sources
        .insert(path.into(), origin.clone());
    Ok(())
}

impl crate::StartupCatalog {
    /// Original checked package declaration for a closed shipped Type or alias.
    pub fn native_type_source(&self, path: &str) -> Option<&NativeTypeSourceOrigin> {
        self.native_type_sources.get(path)
    }

    /// Original checked owner modules retained by a shipped or aliased family.
    /// Lookup aliases and preparation names do not replace this provenance.
    pub fn native_family_sources(
        &self,
        path: &str,
    ) -> Option<impl Iterator<Item = &NativeTypeSourceOrigin>> {
        self.native_families
            .get(path)
            .map(|family| family.source_origins.values())
    }
}

#[cfg(test)]
mod tests;
