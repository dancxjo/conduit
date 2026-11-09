//! Owner-visible recipe pins derived from the same checked Make calibration.
use super::*;
impl Calibration {
    /// Identity of the exact ordered immutable reconstruction recipes. The
    /// source digest must come from the complete authored workset bytes.
    /// This is inert fabrication identity, never a permission to execute.
    pub fn continuation_digest(&self, source_digest: [u8; 32]) -> Result<[u8; 32], String> {
        let placement_ids: Vec<Vec<&str>> = self
            .entries
            .iter()
            .map(|entry| {
                entry
                    .layout
                    .fragment_seal_placement_ids
                    .iter()
                    .map(String::as_str)
                    .collect()
            })
            .collect();
        let entries: Vec<_> = self
            .entries
            .iter()
            .zip(&placement_ids)
            .map(
                |(entry, placement_ids)| conduit_body::ContinuationBundleEntry {
                    name: &entry.name,
                    source_document: entry.source_document_id.as_str(),
                    checked_plot: entry.checked_plot_id.as_str(),
                    expanded_plot: entry.expanded_plot_id.as_str(),
                    projection_digest: entry.layout.projection_template_digest,
                    recipe_digest: c::fragment_seal_recipe_identity(
                        entry.layout.fragment_seal_recipe.pieces(),
                    ),
                    placement_ids,
                    placement_nodes: &entry.layout.fragment_seal_to_kernel,
                },
            )
            .collect();
        conduit_body::continuation_bundle_digest(source_digest, &entries).map_err(debug)
    }

    /// Emit the owner-visible recipe identity from checked fabrication inputs.
    pub fn emit_continuation_identity(
        &self,
        out: &Path,
        source_digest: [u8; 32],
    ) -> Result<(), String> {
        let digest = self.continuation_digest(source_digest)?;
        std::fs::write(out.join("continuation_identity.rs"), format!("// Inert recipe identity; not Start authority.\npub const SOURCE_DIGEST:[u8;32]={source_digest:?};\npub const WORKSET_DIGEST:[u8;32]={digest:?};\n")).map_err(debug)
    }

    pub fn emit_continuation_recipe(
        &self,
        out: &Path,
        source_digest: [u8; 32],
        image_id: &str,
        configuration_id: &str,
    ) -> Result<(), String> {
        use conduit_body::continuation_recipe::{Document, Entry, MAXIMUM_DOCUMENT_BYTES, SCHEMA};
        let document = Document {
            schema: SCHEMA.into(),
            source_digest,
            workset_digest: self.continuation_digest(source_digest)?,
            image_id: image_id.into(),
            configuration_id: configuration_id.into(),
            entries: self
                .entries
                .iter()
                .map(|entry| Entry {
                    name: entry.name.clone(),
                    source_document: entry.source_document_id.as_str().into(),
                    checked_plot: entry.checked_plot_id.as_str().into(),
                    expanded_plot: entry.expanded_plot_id.as_str().into(),
                    projection_digest: entry.layout.projection_template_digest,
                    recipe_digest: c::fragment_seal_recipe_identity(
                        entry.layout.fragment_seal_recipe.pieces(),
                    ),
                    placement_ids: entry.layout.fragment_seal_placement_ids.clone(),
                    placement_nodes: entry.layout.fragment_seal_to_kernel.clone(),
                })
                .collect(),
        };
        document.checked_digest().map_err(debug)?;
        let bytes = serde_json::to_vec(&document).map_err(debug)?;
        if bytes.len() > MAXIMUM_DOCUMENT_BYTES {
            return Err("continuation recipe document exceeds finite bound".into());
        }
        std::fs::write(out.join("continuation-recipe.json"), bytes).map_err(debug)
    }
}
