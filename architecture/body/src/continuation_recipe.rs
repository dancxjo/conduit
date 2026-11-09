//! Inert calibrated Make pins. A decoded document is not a sealed artifact,
//! provider possession, checked Source authority or execution permission.
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
pub const SCHEMA: &str = "conduit.make/continuation-recipe@1";
pub const MAXIMUM_DOCUMENT_BYTES: usize = 131_072;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema: String,
    pub image_id: String,
    pub configuration_id: String,
    pub source_digest: [u8; 32],
    pub workset_digest: [u8; 32],
    pub entries: Vec<Entry>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub source_document: String,
    pub checked_plot: String,
    pub expanded_plot: String,
    pub projection_digest: [u8; 32],
    pub recipe_digest: [u8; 32],
    pub placement_ids: Vec<String>,
    pub placement_nodes: Vec<u16>,
}
impl Document {
    /// Content consistency only. The caller must independently verify these
    /// exact bytes through the original checked Make bundle before policy use.
    pub fn checked_digest(&self) -> Result<[u8; 32], crate::ContinuationBundleRefusal> {
        if self.image_id.is_empty()
            || self.configuration_id.is_empty()
            || self.image_id.len() > crate::MAX_LIFECYCLE_ID_BYTES
            || self.configuration_id.len() > crate::MAX_LIFECYCLE_ID_BYTES
            || self.schema != SCHEMA
            || self.entries.is_empty()
            || self.entries.len() > 16
        {
            return Err(crate::ContinuationBundleRefusal::Bounds);
        }
        let placements: Vec<Vec<&str>> = self
            .entries
            .iter()
            .map(|e| e.placement_ids.iter().map(String::as_str).collect())
            .collect();
        let entries: Vec<_> = self
            .entries
            .iter()
            .zip(&placements)
            .map(|(entry, placements)| crate::ContinuationBundleEntry {
                name: &entry.name,
                source_document: &entry.source_document,
                checked_plot: &entry.checked_plot,
                expanded_plot: &entry.expanded_plot,
                projection_digest: entry.projection_digest,
                recipe_digest: entry.recipe_digest,
                placement_ids: placements,
                placement_nodes: &entry.placement_nodes,
            })
            .collect();
        let digest = crate::continuation_bundle_digest(self.source_digest, &entries)?;
        if digest != self.workset_digest {
            return Err(crate::ContinuationBundleRefusal::Workset);
        }
        Ok(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_recipe_transcript_refuses_changed_mapping_and_unpinned_workset() {
        let entry = Entry {
            name: "entry".into(),
            source_document: "source".into(),
            checked_plot: "checked".into(),
            expanded_plot: "expanded".into(),
            projection_digest: [2; 32],
            recipe_digest: [3; 32],
            placement_ids: alloc::vec!["placement".into()],
            placement_nodes: alloc::vec![0],
        };
        let placements = ["placement"];
        let pin = crate::ContinuationBundleEntry {
            name: &entry.name,
            source_document: &entry.source_document,
            checked_plot: &entry.checked_plot,
            expanded_plot: &entry.expanded_plot,
            projection_digest: entry.projection_digest,
            recipe_digest: entry.recipe_digest,
            placement_ids: &placements,
            placement_nodes: &entry.placement_nodes,
        };
        let digest = crate::continuation_bundle_digest([1; 32], &[pin]).unwrap();
        let mut document = Document {
            schema: SCHEMA.into(),
            image_id: "image".into(),
            configuration_id: "configuration".into(),
            source_digest: [1; 32],
            workset_digest: digest,
            entries: alloc::vec![entry],
        };
        assert_eq!(document.checked_digest().unwrap(), digest);
        document.entries[0].placement_nodes[0] = 1;
        assert!(matches!(
            document.checked_digest(),
            Err(crate::ContinuationBundleRefusal::Workset)
        ));
        document.entries[0].placement_nodes.clear();
        assert!(matches!(
            document.checked_digest(),
            Err(crate::ContinuationBundleRefusal::Bounds)
        ));
    }
}
