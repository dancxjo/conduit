use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::PortDirection;

use super::{
    validate_identity, MaskBoundaryPort, MaskBoundaryRole, MaskCordSpecification,
    MaskSpecificationError, MaskStageId, MaskStageSpecification,
};
use crate::{
    MANIFESTATION_VALUE_KIND, PRESENTATION_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND,
};

pub(super) fn validate_stages(
    stages: &[MaskStageSpecification],
) -> Result<(), MaskSpecificationError> {
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

pub(super) fn validate_cords(
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

pub(super) fn validate_topology(
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

pub(super) fn validate_boundaries(
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
