//! Durable bounded Patchbay presentation state, separate from authored Form meaning.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::{
    PatchbayGraph, PatchbayGraphError, PatchbaySubjectRef, MAX_PATCHBAY_CORDS, MAX_PATCHBAY_GEARS,
};

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

/// One finite presentation-only waypoint for an exact semantic Cord.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CordRoute {
    pub source_port_identity: String,
    pub sink_port_identity: String,
    pub bend_x: i32,
    pub bend_y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayLayout {
    pub version: u8,
    pub gears: Vec<GearPlacement>,
    #[serde(default)]
    pub cords: Vec<CordRoute>,
    /// Gear reverse visibility is presentation state, never Form or Plan truth.
    #[serde(default)]
    pub reversed_gears: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayLayoutError {
    WrongVersion,
    TooManyGears,
    TooManyCords,
    CoordinateOutOfBounds,
    GroupNameTooLarge,
    StaleGraphBasis,
    UnknownGear,
    DuplicateGearPlacement,
    DuplicateCordRoute,
    UnknownCord,
}

impl Default for PatchbayLayout {
    fn default() -> Self {
        Self {
            version: PATCHBAY_LAYOUT_VERSION,
            gears: Vec::with_capacity(MAX_PATCHBAY_GEARS),
            cords: Vec::with_capacity(MAX_PATCHBAY_CORDS),
            reversed_gears: Vec::with_capacity(MAX_PATCHBAY_GEARS),
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
        if self.cords.len() > MAX_PATCHBAY_CORDS {
            return Err(PatchbayLayoutError::TooManyCords);
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
        let mut cord_keys = BTreeSet::new();
        for route in &self.cords {
            if route.source_port_identity.is_empty()
                || route.sink_port_identity.is_empty()
                || !cord_keys.insert((
                    route.source_port_identity.as_str(),
                    route.sink_port_identity.as_str(),
                ))
            {
                return Err(PatchbayLayoutError::DuplicateCordRoute);
            }
            validate_coordinate(route.bend_x, route.bend_y)?;
        }
        let mut reversed = BTreeSet::new();
        if self.reversed_gears.len() > MAX_PATCHBAY_GEARS
            || self
                .reversed_gears
                .iter()
                .any(|identity| identity.is_empty() || !reversed.insert(identity.as_str()))
        {
            return Err(PatchbayLayoutError::DuplicateGearPlacement);
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

    pub fn is_reversed(&self, gear_identity: &str) -> bool {
        self.reversed_gears
            .iter()
            .any(|identity| identity == gear_identity)
    }

    pub fn flip_gear(
        &mut self,
        graph: &PatchbayGraph,
        gear: &PatchbaySubjectRef,
    ) -> Result<bool, PatchbayLayoutError> {
        validate_gear(graph, gear)?;
        if let Some(index) = self
            .reversed_gears
            .iter()
            .position(|identity| identity == &gear.subject_identity)
        {
            self.reversed_gears.remove(index);
            Ok(false)
        } else {
            if self.reversed_gears.len() == MAX_PATCHBAY_GEARS {
                return Err(PatchbayLayoutError::TooManyGears);
            }
            self.reversed_gears.push(gear.subject_identity.clone());
            Ok(true)
        }
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

    pub fn cord_route(&self, source_port: &str, sink_port: &str) -> Option<(i32, i32)> {
        self.cords
            .iter()
            .find(|route| {
                route.source_port_identity == source_port && route.sink_port_identity == sink_port
            })
            .map(|route| (route.bend_x, route.bend_y))
    }

    /// Changes only durable presentation geometry. The exact Cord must already
    /// exist in the checked graph; no authored connectivity is created here.
    pub fn route_cord(
        &mut self,
        graph: &PatchbayGraph,
        cord: &PatchbaySubjectRef,
        bend_x: i32,
        bend_y: i32,
    ) -> Result<(), PatchbayLayoutError> {
        if cord.expanded_form_id != graph.expanded_form_id {
            return Err(PatchbayLayoutError::StaleGraphBasis);
        }
        let cord = graph
            .cords
            .iter()
            .find(|candidate| candidate.identity == cord.subject_identity)
            .ok_or(PatchbayLayoutError::UnknownCord)?;
        validate_coordinate(bend_x, bend_y)?;
        if let Some(route) = self.cords.iter_mut().find(|route| {
            route.source_port_identity == cord.source_port
                && route.sink_port_identity == cord.sink_port
        }) {
            route.bend_x = bend_x;
            route.bend_y = bend_y;
            return Ok(());
        }
        if self.cords.len() == MAX_PATCHBAY_CORDS {
            return Err(PatchbayLayoutError::TooManyCords);
        }
        self.cords.push(CordRoute {
            source_port_identity: cord.source_port.clone(),
            sink_port_identity: cord.sink_port.clone(),
            bend_x,
            bend_y,
        });
        Ok(())
    }

    pub fn reconcile(&mut self, graph: &PatchbayGraph) {
        self.gears.retain(|placement| {
            graph
                .gears
                .iter()
                .any(|gear| gear.identity == placement.gear_identity)
        });
        self.cords.retain(|route| {
            graph.cords.iter().any(|cord| {
                cord.source_port == route.source_port_identity
                    && cord.sink_port == route.sink_port_identity
            })
        });
        self.reversed_gears
            .retain(|identity| graph.gears.iter().any(|gear| &gear.identity == identity));
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

    #[test]
    fn cord_waypoint_round_trip_is_bounded_presentation_only() {
        let editor = FormEditor::from_source(
            PathBuf::from("route.conduit"),
            "form route {\n    literal: text/literal(\"hello\")\n    upper: text/upper\n    literal.text > upper.text\n}\n".into(),
        )
        .unwrap();
        let graph = PatchbayGraph::from_expanded(&editor.expand_form("route").unwrap()).unwrap();
        let identities = (
            graph.source_document_id.clone(),
            graph.checked_form_id.clone(),
            graph.expanded_form_id.clone(),
            graph.cords[0].identity.clone(),
        );
        let cord = graph.subject_ref(&graph.cords[0].identity).unwrap();
        let mut layout = PatchbayLayout::default();
        layout.route_cord(&graph, &cord, 420, 240).unwrap();
        let encoded = serde_json::to_vec(&layout).unwrap();
        let reopened: PatchbayLayout = serde_json::from_slice(&encoded).unwrap();
        reopened.validate().unwrap();
        assert_eq!(
            reopened.cord_route(&graph.cords[0].source_port, &graph.cords[0].sink_port),
            Some((420, 240))
        );
        assert_eq!(
            identities,
            (
                graph.source_document_id,
                graph.checked_form_id,
                graph.expanded_form_id,
                graph.cords[0].identity.clone()
            )
        );
    }
}
