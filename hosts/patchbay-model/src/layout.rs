//! Durable bounded Patchbay presentation state, separate from authored Form meaning.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::{PatchbayGraph, PatchbayGraphError, PatchbaySubjectRef, MAX_PATCHBAY_GEARS};

pub const PATCHBAY_LAYOUT_VERSION: u8 = 1;
pub const MAX_LAYOUT_COORDINATE: i32 = 32_767;
pub const MAX_GROUP_NAME_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GearPlacement {
    pub gear_identity: String,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub positioned: bool,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayLayout {
    pub version: u8,
    pub gears: Vec<GearPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayLayoutError {
    WrongVersion,
    TooManyGears,
    CoordinateOutOfBounds,
    GroupNameTooLarge,
    StaleGraphBasis,
    UnknownGear,
    DuplicateGearPlacement,
}

impl Default for PatchbayLayout {
    fn default() -> Self {
        Self {
            version: PATCHBAY_LAYOUT_VERSION,
            gears: Vec::with_capacity(MAX_PATCHBAY_GEARS),
        }
    }
}

impl PatchbayLayout {
    pub fn validate(&self) -> Result<(), PatchbayLayoutError> {
        if self.version != PATCHBAY_LAYOUT_VERSION {
            return Err(PatchbayLayoutError::WrongVersion);
        }
        if self.gears.len() > MAX_PATCHBAY_GEARS {
            return Err(PatchbayLayoutError::TooManyGears);
        }
        let mut identities = BTreeSet::new();
        for placement in &self.gears {
            if placement.gear_identity.is_empty()
                || !identities.insert(placement.gear_identity.as_str())
            {
                return Err(PatchbayLayoutError::DuplicateGearPlacement);
            }
            validate_coordinate(placement.x, placement.y)?;
            if placement
                .group
                .as_ref()
                .is_some_and(|group| group.len() > MAX_GROUP_NAME_BYTES)
            {
                return Err(PatchbayLayoutError::GroupNameTooLarge);
            }
        }
        Ok(())
    }

    pub fn position(&self, gear_identity: &str) -> Option<(i32, i32)> {
        self.gears
            .iter()
            .find(|placement| placement.gear_identity == gear_identity)
            .filter(|placement| placement.positioned)
            .map(|placement| (placement.x, placement.y))
    }

    pub fn move_gear(
        &mut self,
        graph: &PatchbayGraph,
        gear: &PatchbaySubjectRef,
        x: i32,
        y: i32,
    ) -> Result<(), PatchbayLayoutError> {
        validate_gear(graph, gear)?;
        validate_coordinate(x, y)?;
        if let Some(placement) = self
            .gears
            .iter_mut()
            .find(|placement| placement.gear_identity == gear.subject_identity)
        {
            placement.x = x;
            placement.y = y;
            placement.positioned = true;
            return Ok(());
        }
        if self.gears.len() == MAX_PATCHBAY_GEARS {
            return Err(PatchbayLayoutError::TooManyGears);
        }
        self.gears.push(GearPlacement {
            gear_identity: gear.subject_identity.clone(),
            x,
            y,
            positioned: true,
            group: None,
        });
        Ok(())
    }

    pub fn group_gear(
        &mut self,
        graph: &PatchbayGraph,
        gear: &PatchbaySubjectRef,
        group: Option<String>,
    ) -> Result<(), PatchbayLayoutError> {
        validate_gear(graph, gear)?;
        if group
            .as_ref()
            .is_some_and(|value| value.len() > MAX_GROUP_NAME_BYTES)
        {
            return Err(PatchbayLayoutError::GroupNameTooLarge);
        }
        if let Some(placement) = self
            .gears
            .iter_mut()
            .find(|placement| placement.gear_identity == gear.subject_identity)
        {
            placement.group = group;
        } else {
            if self.gears.len() == MAX_PATCHBAY_GEARS {
                return Err(PatchbayLayoutError::TooManyGears);
            }
            self.gears.push(GearPlacement {
                gear_identity: gear.subject_identity.clone(),
                x: 0,
                y: 0,
                positioned: false,
                group,
            });
        }
        Ok(())
    }

    pub fn reconcile(&mut self, graph: &PatchbayGraph) {
        self.gears.retain(|placement| {
            graph
                .gears
                .iter()
                .any(|gear| gear.identity == placement.gear_identity)
        });
    }
}

fn validate_coordinate(x: i32, y: i32) -> Result<(), PatchbayLayoutError> {
    if x.unsigned_abs() > MAX_LAYOUT_COORDINATE as u32
        || y.unsigned_abs() > MAX_LAYOUT_COORDINATE as u32
    {
        return Err(PatchbayLayoutError::CoordinateOutOfBounds);
    }
    Ok(())
}

fn validate_gear(
    graph: &PatchbayGraph,
    subject: &PatchbaySubjectRef,
) -> Result<(), PatchbayLayoutError> {
    graph
        .resolve_subject_ref(subject)
        .map_err(|error| match error {
            PatchbayGraphError::StaleGraphBasis => PatchbayLayoutError::StaleGraphBasis,
            _ => PatchbayLayoutError::UnknownGear,
        })?;
    if !graph
        .gears
        .iter()
        .any(|gear| gear.identity == subject.subject_identity)
    {
        return Err(PatchbayLayoutError::UnknownGear);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FormEditor;
    use std::path::PathBuf;

    #[test]
    fn movement_grouping_and_round_trip_never_change_graph_identity() {
        let editor = FormEditor::from_source(
            PathBuf::from("count.conduit"),
            include_str!("../../../examples/count.conduit").into(),
        )
        .unwrap();
        let graph =
            PatchbayGraph::from_expanded(&editor.expand_form("count-demo").unwrap()).unwrap();
        let identities = (
            graph.source_document_id.clone(),
            graph.checked_form_id.clone(),
            graph.expanded_form_id.clone(),
        );
        let gear = graph.subject_ref(&graph.gears[0].identity).unwrap();
        let mut layout = PatchbayLayout::default();
        layout.move_gear(&graph, &gear, 420, 180).unwrap();
        layout
            .group_gear(&graph, &gear, Some("sensing".into()))
            .unwrap();
        let encoded = serde_json::to_vec(&layout).unwrap();
        let reopened: PatchbayLayout = serde_json::from_slice(&encoded).unwrap();
        reopened.validate().unwrap();
        assert_eq!(reopened.position(&gear.subject_identity), Some((420, 180)));
        assert_eq!(
            identities,
            (
                graph.source_document_id,
                graph.checked_form_id,
                graph.expanded_form_id
            )
        );
    }
}
