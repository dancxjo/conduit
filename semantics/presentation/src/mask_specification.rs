//! Portable Mask meaning and its exact admitted realization topology.
//!
//! A specification owns typed realization intent. A [`PlannedMask`] binds that
//! intent to one immutable ordinary Plan without promoting Hosts, artifacts,
//! resources, or intermediate values into Face truth.

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use conduit_core::{
    verify_plan, AdmittedLine, ArtifactId, BootId, CapabilityId, ConnectionId, HostId,
    ImplementationId, KindId, KindIdentity, PlacementId, Plan, PlanId, PortDescriptor,
    PortDirection, PortId, ResourceBinding,
};
use serde::{Deserialize, Serialize};

#[path = "mask_specification/identity.rs"]
mod identity;
use identity::{bind_specification, validate_identity};

use crate::{
    MANIFESTATION_VALUE_KIND, PRESENTATION_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND,
};

pub const MAX_MASK_STAGES: usize = 16;
pub const MAX_MASK_CORDS: usize = 32;
pub const MAX_MASK_BOUNDARIES: usize = 8;
pub const MAX_MASK_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MaskSpecificationId(String);

impl MaskSpecificationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MaskStageId(String);

impl MaskStageId {
    pub fn new(value: impl Into<String>) -> Result<Self, MaskSpecificationError> {
        let value = value.into();
        validate_identity(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskStageSpecification {
    pub stage_id: MaskStageId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskCordSpecification {
    pub source_stage_id: MaskStageId,
    pub source_port_id: PortId,
    pub sink_stage_id: MaskStageId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskBoundaryRole {
    PresentationInput,
    ShowOutput,
    LocalInteractionInput,
    FaceInteractionOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskBoundaryPort {
    pub role: MaskBoundaryRole,
    pub stage_id: MaskStageId,
    pub port_id: PortId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskSpecification {
    pub specification_id: MaskSpecificationId,
    pub name: String,
    pub revision: u64,
    pub stages: Vec<MaskStageSpecification>,
    pub cords: Vec<MaskCordSpecification>,
    pub boundaries: Vec<MaskBoundaryPort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskStagePlacement {
    pub stage_id: MaskStageId,
    pub placement_id: PlacementId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMask {
    pub specification_id: MaskSpecificationId,
    pub specification_revision: u64,
    pub plan_id: PlanId,
    pub stage_placements: Vec<MaskStagePlacement>,
    pub stages: Vec<PlannedMaskStage>,
    pub cords: Vec<PlannedMaskCord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMaskStage {
    pub stage_id: MaskStageId,
    pub placement_id: PlacementId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub resources: Vec<ResourceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMaskCord {
    pub source_stage_id: MaskStageId,
    pub sink_stage_id: MaskStageId,
    pub connection_id: ConnectionId,
    pub selected_line: Option<AdmittedLine>,
    pub admitted_lines: Vec<AdmittedLine>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskSpecificationError {
    InvalidIdentity,
    StageCapacityExceeded,
    CordCapacityExceeded,
    BoundaryCapacityExceeded,
    DuplicateStage,
    DuplicateCord,
    DuplicateBoundary,
    UnknownStage,
    UnknownPort,
    WrongPortDirection,
    IncompatibleCord,
    CyclicTopology,
    OrphanStage,
    MissingPresentationInput,
    MultiplePresentationInputs,
    MissingShowOutput,
    MultipleShowOutputs,
    IncompleteInteractionBoundary,
    InvalidPlan,
    IncompleteStagePlacement,
    DuplicateStagePlacement,
    WrongStageKind,
    WrongStageContract,
    MissingPlannedCord,
    UnexpectedPlannedStage,
    UnexpectedPlannedCord,
}

impl MaskSpecification {
    pub fn new(
        name: impl Into<String>,
        revision: u64,
        stages: Vec<MaskStageSpecification>,
        cords: Vec<MaskCordSpecification>,
        boundaries: Vec<MaskBoundaryPort>,
    ) -> Result<Self, MaskSpecificationError> {
        let name = name.into();
        validate_identity(&name)?;
        if stages.is_empty() || stages.len() > MAX_MASK_STAGES {
            return Err(MaskSpecificationError::StageCapacityExceeded);
        }
        if cords.len() > MAX_MASK_CORDS {
            return Err(MaskSpecificationError::CordCapacityExceeded);
        }
        if boundaries.len() > MAX_MASK_BOUNDARIES {
            return Err(MaskSpecificationError::BoundaryCapacityExceeded);
        }
        validate_stages(&stages)?;
        validate_cords(&stages, &cords)?;
        validate_boundaries(&stages, &boundaries)?;
        validate_topology(&stages, &cords, &boundaries)?;
        let specification_id = bind_specification(&name, revision, &stages, &cords, &boundaries);
        Ok(Self {
            specification_id,
            name,
            revision,
            stages,
            cords,
            boundaries,
        })
    }

    pub fn admit_plan(
        &self,
        plan: &Plan,
        mut stage_placements: Vec<MaskStagePlacement>,
    ) -> Result<PlannedMask, MaskSpecificationError> {
        if !verify_plan(plan) {
            return Err(MaskSpecificationError::InvalidPlan);
        }
        if stage_placements.len() != self.stages.len() {
            return Err(MaskSpecificationError::IncompleteStagePlacement);
        }
        stage_placements.sort_by(|left, right| left.stage_id.cmp(&right.stage_id));
        if stage_placements.windows(2).any(|pair| {
            pair[0].stage_id == pair[1].stage_id || pair[0].placement_id == pair[1].placement_id
        }) {
            return Err(MaskSpecificationError::DuplicateStagePlacement);
        }
        let placements = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| (placement.placement_id.clone(), placement))
            .collect::<BTreeMap<_, _>>();
        if placements.len() != self.stages.len() {
            return Err(MaskSpecificationError::UnexpectedPlannedStage);
        }
        let bindings = stage_placements
            .iter()
            .map(|binding| (binding.stage_id.clone(), binding))
            .collect::<BTreeMap<_, _>>();
        for stage in &self.stages {
            let binding = bindings
                .get(&stage.stage_id)
                .ok_or(MaskSpecificationError::IncompleteStagePlacement)?;
            let placement = placements
                .get(&binding.placement_id)
                .ok_or(MaskSpecificationError::IncompleteStagePlacement)?;
            if placement.kind_id != stage.kind_id {
                return Err(MaskSpecificationError::WrongStageKind);
            }
            if placement.kind_contract_revision != stage.kind_contract_revision
                || placement.inputs != stage.inputs
                || placement.outputs != stage.outputs
            {
                return Err(MaskSpecificationError::WrongStageContract);
            }
        }
        let planned_connections = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .collect::<Vec<_>>();
        let mut admitted_cords = Vec::with_capacity(self.cords.len());
        for cord in &self.cords {
            let source = &bindings[&cord.source_stage_id].placement_id;
            let sink = &bindings[&cord.sink_stage_id].placement_id;
            let planned = planned_connections
                .iter()
                .find(|planned| {
                    &planned.source_placement_id == source
                        && planned.source_port_id == cord.source_port_id
                        && &planned.sink_placement_id == sink
                        && planned.sink_port_id == cord.sink_port_id
                        && planned.value_kind == cord.value_kind
                })
                .ok_or(MaskSpecificationError::MissingPlannedCord)?;
            admitted_cords.push(PlannedMaskCord {
                source_stage_id: cord.source_stage_id.clone(),
                sink_stage_id: cord.sink_stage_id.clone(),
                connection_id: planned.connection_id.clone(),
                selected_line: planned.selected_line.clone(),
                admitted_lines: planned.admitted_lines.clone(),
                item_capacity: planned.item_capacity,
                byte_capacity: planned.byte_capacity,
            });
        }
        if plan
            .fragments
            .iter()
            .map(|fragment| fragment.connections.len())
            .sum::<usize>()
            != self.cords.len()
        {
            return Err(MaskSpecificationError::UnexpectedPlannedCord);
        }
        let admitted_stages = self
            .stages
            .iter()
            .map(|stage| {
                let placement = placements[&bindings[&stage.stage_id].placement_id];
                PlannedMaskStage {
                    stage_id: stage.stage_id.clone(),
                    placement_id: placement.placement_id.clone(),
                    capability_id: placement.capability_id.clone(),
                    implementation_id: placement.implementation_id.clone(),
                    artifact_id: placement.artifact_id.clone(),
                    host_id: placement.host_id.clone(),
                    boot_id: placement.boot_id.clone(),
                    resources: placement.resources.clone(),
                }
            })
            .collect();
        Ok(PlannedMask {
            specification_id: self.specification_id.clone(),
            specification_revision: self.revision,
            plan_id: plan.plan_id.clone(),
            stage_placements,
            stages: admitted_stages,
            cords: admitted_cords,
        })
    }
}

fn validate_stages(stages: &[MaskStageSpecification]) -> Result<(), MaskSpecificationError> {
    for (index, stage) in stages.iter().enumerate() {
        validate_identity(stage.stage_id.as_str())?;
        if stages[index + 1..]
            .iter()
            .any(|other| other.stage_id == stage.stage_id)
        {
            return Err(MaskSpecificationError::DuplicateStage);
        }
        if stage
            .inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
            || stage
                .outputs
                .iter()
                .any(|port| port.direction != PortDirection::Output)
        {
            return Err(MaskSpecificationError::WrongPortDirection);
        }
    }
    Ok(())
}

fn validate_cords(
    stages: &[MaskStageSpecification],
    cords: &[MaskCordSpecification],
) -> Result<(), MaskSpecificationError> {
    for (index, cord) in cords.iter().enumerate() {
        if cords[index + 1..].contains(cord) {
            return Err(MaskSpecificationError::DuplicateCord);
        }
        let source = stage(stages, &cord.source_stage_id)?;
        let sink = stage(stages, &cord.sink_stage_id)?;
        let output = source
            .outputs
            .iter()
            .find(|port| port.port_id == cord.source_port_id)
            .ok_or(MaskSpecificationError::UnknownPort)?;
        let input = sink
            .inputs
            .iter()
            .find(|port| port.port_id == cord.sink_port_id)
            .ok_or(MaskSpecificationError::UnknownPort)?;
        if output.value_kind != cord.value_kind || input.value_kind != cord.value_kind {
            return Err(MaskSpecificationError::IncompatibleCord);
        }
    }
    Ok(())
}

fn validate_topology(
    stages: &[MaskStageSpecification],
    cords: &[MaskCordSpecification],
    boundaries: &[MaskBoundaryPort],
) -> Result<(), MaskSpecificationError> {
    let mut incoming = stages
        .iter()
        .map(|stage| (stage.stage_id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<MaskStageId, Vec<MaskStageId>>::new();
    for cord in cords {
        *incoming
            .get_mut(&cord.sink_stage_id)
            .ok_or(MaskSpecificationError::UnknownStage)? += 1;
        outgoing
            .entry(cord.source_stage_id.clone())
            .or_default()
            .push(cord.sink_stage_id.clone());
    }
    if stages.iter().any(|stage| {
        incoming[&stage.stage_id] == 0
            && !outgoing.contains_key(&stage.stage_id)
            && !boundaries
                .iter()
                .any(|boundary| boundary.stage_id == stage.stage_id)
    }) {
        return Err(MaskSpecificationError::OrphanStage);
    }
    let mut ready = incoming
        .iter()
        .filter_map(|(stage, count)| (*count == 0).then_some(stage.clone()))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some(stage) = ready.pop() {
        visited += 1;
        if let Some(successors) = outgoing.get(&stage) {
            for successor in successors {
                let count = incoming
                    .get_mut(successor)
                    .ok_or(MaskSpecificationError::UnknownStage)?;
                *count -= 1;
                if *count == 0 {
                    ready.push(successor.clone());
                }
            }
        }
    }
    if visited != stages.len() {
        return Err(MaskSpecificationError::CyclicTopology);
    }
    Ok(())
}

fn validate_boundaries(
    stages: &[MaskStageSpecification],
    boundaries: &[MaskBoundaryPort],
) -> Result<(), MaskSpecificationError> {
    if boundaries.iter().enumerate().any(|(index, boundary)| {
        boundaries[index + 1..].iter().any(|other| {
            other.role == boundary.role
                && other.stage_id == boundary.stage_id
                && other.port_id == boundary.port_id
        })
    }) {
        return Err(MaskSpecificationError::DuplicateBoundary);
    }
    let mut presentations = 0;
    let mut shows = 0;
    let mut local_interactions = 0;
    let mut face_interactions = 0;
    for boundary in boundaries {
        let stage = stage(stages, &boundary.stage_id)?;
        let (ports, expected_direction, expected_kind) = match boundary.role {
            MaskBoundaryRole::PresentationInput => {
                presentations += 1;
                (
                    &stage.inputs,
                    PortDirection::Input,
                    Some(PRESENTATION_VALUE_KIND),
                )
            }
            MaskBoundaryRole::ShowOutput => {
                shows += 1;
                (
                    &stage.outputs,
                    PortDirection::Output,
                    Some(MANIFESTATION_VALUE_KIND),
                )
            }
            MaskBoundaryRole::LocalInteractionInput => {
                local_interactions += 1;
                (&stage.inputs, PortDirection::Input, None)
            }
            MaskBoundaryRole::FaceInteractionOutput => {
                face_interactions += 1;
                (
                    &stage.outputs,
                    PortDirection::Output,
                    Some(PRESENTATION_INTERACTION_VALUE_KIND),
                )
            }
        };
        let port = ports
            .iter()
            .find(|port| port.port_id == boundary.port_id)
            .ok_or(MaskSpecificationError::UnknownPort)?;
        if port.direction != expected_direction {
            return Err(MaskSpecificationError::WrongPortDirection);
        }
        if expected_kind.is_some_and(|kind| port.value_kind.as_str() != kind) {
            return Err(MaskSpecificationError::IncompatibleCord);
        }
    }
    match presentations {
        0 => return Err(MaskSpecificationError::MissingPresentationInput),
        1 => {}
        _ => return Err(MaskSpecificationError::MultiplePresentationInputs),
    }
    match shows {
        0 => return Err(MaskSpecificationError::MissingShowOutput),
        1 => {}
        _ => return Err(MaskSpecificationError::MultipleShowOutputs),
    }
    if (local_interactions == 0) != (face_interactions == 0) {
        return Err(MaskSpecificationError::IncompleteInteractionBoundary);
    }
    Ok(())
}

fn stage<'a>(
    stages: &'a [MaskStageSpecification],
    id: &MaskStageId,
) -> Result<&'a MaskStageSpecification, MaskSpecificationError> {
    stages
        .iter()
        .find(|stage| &stage.stage_id == id)
        .ok_or(MaskSpecificationError::UnknownStage)
}
