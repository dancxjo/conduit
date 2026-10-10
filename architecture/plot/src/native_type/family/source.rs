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

impl crate::StartupCatalog {
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
