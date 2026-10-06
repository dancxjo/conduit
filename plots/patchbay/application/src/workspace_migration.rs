//! Only the old Flow presentation is migrated; authored Plot source is never an input to mutation.
use alloc::{collections::BTreeSet, string::String, vec, vec::Vec};
use serde::Deserialize;

use crate::workspace::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LegacyFlowPresentation {
    pub schema: String,
    pub workspace_identity: String,
    pub nodes: Vec<LegacyFlowNode>,
    pub viewport: WorkspaceViewport,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyFlowNode {
    pub id: String,
    pub position: LegacyFlowPoint,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyFlowPoint {
    pub x: f64,
    pub y: f64,
}

impl LegacyFlowPresentation {
    /// Mapping must come from the exact current projection's `semantic-id` properties.
    /// Missing mappings refuse migration, rather than treating a renderer ID as semantic.
    pub fn migrate(
        &self,
        expected_workspace_identity: &str,
        basis: WorkspaceBasis,
        semantic_ids: &[(String, String)],
    ) -> Result<PatchbayWorkspace, WorkspaceError> {
        if self.schema != "conduit.patchbay.flow-presentation/v1" {
            return Err(WorkspaceError::UnsupportedSchema);
        }
        if self.workspace_identity != expected_workspace_identity {
            return Err(WorkspaceError::ChangedBasis);
        }
        if self.nodes.len() > MAX_WORKSPACE_SUBJECTS {
            return Err(WorkspaceError::BoundExceeded);
        }
        let mut seen = BTreeSet::new();
        let mut positions = Vec::new();
        for node in &self.nodes {
            if !seen.insert(&node.id) {
                return Err(WorkspaceError::DuplicateIdentity);
            }
            let mut matches = semantic_ids.iter().filter(|(id, _)| id == &node.id);
            let (_, subject) = matches
                .next()
                .ok_or(WorkspaceError::UnmappedLegacySubject)?;
            if matches.next().is_some() {
                return Err(WorkspaceError::DuplicateIdentity);
            }
            if !node.position.x.is_finite()
                || !node.position.y.is_finite()
                || node.position.x.abs() > 32767.0
                || node.position.y.abs() > 32767.0
            {
                return Err(WorkspaceError::InvalidGeometry);
            }
            positions.push(WorkspacePosition {
                subject: subject.clone(),
                // Truncation is defined identically for browser and native migration.
                x: node.position.x as i32,
                y: node.position.y as i32,
            });
        }
        positions.sort_by(|a, b| a.subject.cmp(&b.subject));
        let workspace = PatchbayWorkspace {
            schema: PATCHBAY_WORKSPACE_SCHEMA.into(),
            basis,
            active_layout: "Default".into(),
            layouts: vec![WorkspaceLayout {
                name: "Default".into(),
                positions,
                routes: vec![],
                frames: vec![],
                notes: vec![],
                collapsed: vec![],
                viewport: self.viewport,
                lens: WorkspaceLens::World,
            }],
        };
        workspace.validate()?;
        Ok(workspace)
    }
}
