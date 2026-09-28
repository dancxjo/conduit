//! Mask chains derived from ordinary immutable Plan placements and Cords.

use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::{
    verify_plan, BootId, CapabilityId, HostId, ImplementationId, PlacementId, Plan, PlanId,
    PlannedGear, ResourceBinding,
};
use serde::{Deserialize, Serialize};

use crate::{PRESENTATION_VALUE_KIND, PRESENTER_STAGE_KIND, RENDERER_KIND};

pub const MAX_MASK_CHAINS: usize = 8;
pub const MAX_MASK_STAGES_PER_CHAIN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMaskStage {
    pub placement_id: PlacementId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub input_kind: conduit_core::KindId,
    pub output_kind: conduit_core::KindId,
    pub input_item_capacity: u16,
    pub input_byte_capacity: u32,
    pub resources: Vec<ResourceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMaskChain {
    pub stages: Vec<PlannedMaskStage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskTopologyAdmission {
    pub plan_id: PlanId,
    pub chains: Vec<PlannedMaskChain>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskTopologyError {
    InvalidPlan,
    InvalidStageContract,
    IncompatibleType,
    Cycle,
    ChainLengthBound,
    ParallelChainBound,
    MissingTerminalRenderer,
    DuplicateStage,
}

impl MaskTopologyAdmission {
    pub fn from_plan(plan: &Plan) -> Result<Self, MaskTopologyError> {
        if !verify_plan(plan) {
            return Err(MaskTopologyError::InvalidPlan);
        }
        let placements = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .map(|p| (p.placement_id.clone(), p))
            .collect::<BTreeMap<_, _>>();
        let mut next = BTreeMap::<PlacementId, Vec<PlacementId>>::new();
        let mut incoming = BTreeMap::<PlacementId, usize>::new();
        for fragment in &plan.fragments {
            for cord in &fragment.connections {
                if cord.value_kind.as_str() != PRESENTATION_VALUE_KIND {
                    continue;
                }
                let Some(source) = placements.get(&cord.source_placement_id) else {
                    continue;
                };
                let Some(sink) = placements.get(&cord.sink_placement_id) else {
                    continue;
                };
                if is_mask_stage(source) && is_mask_stage(sink) {
                    next.entry(source.placement_id.clone())
                        .or_default()
                        .push(sink.placement_id.clone());
                    *incoming.entry(sink.placement_id.clone()).or_default() += 1;
                }
            }
        }
        let roots = placements
            .values()
            .filter(|p| {
                is_mask_stage(p) && incoming.get(&p.placement_id).copied().unwrap_or(0) == 0
            })
            .collect::<Vec<_>>();
        let mut chains = Vec::new();
        for root in roots {
            walk(root, &placements, &next, &mut Vec::new(), &mut chains)?;
        }
        if chains.len() > MAX_MASK_CHAINS {
            return Err(MaskTopologyError::ParallelChainBound);
        }
        if !placements.values().any(|p| is_mask_stage(p)) {
            return Ok(Self {
                plan_id: plan.plan_id.clone(),
                chains,
            });
        }
        let visited = chains
            .iter()
            .flat_map(|c| &c.stages)
            .map(|s| &s.placement_id)
            .collect::<alloc::collections::BTreeSet<_>>();
        if placements
            .values()
            .filter(|p| is_mask_stage(p))
            .any(|p| !visited.contains(&p.placement_id))
        {
            return Err(MaskTopologyError::Cycle);
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            chains,
        })
    }
}

fn walk(
    placement: &PlannedGear,
    placements: &BTreeMap<PlacementId, &PlannedGear>,
    next: &BTreeMap<PlacementId, Vec<PlacementId>>,
    path: &mut Vec<PlacementId>,
    chains: &mut Vec<PlannedMaskChain>,
) -> Result<(), MaskTopologyError> {
    if path.contains(&placement.placement_id) {
        return Err(MaskTopologyError::Cycle);
    }
    if path.len() >= MAX_MASK_STAGES_PER_CHAIN {
        return Err(MaskTopologyError::ChainLengthBound);
    }
    validate_stage(placement)?;
    path.push(placement.placement_id.clone());
    if placement.kind_id.as_str() == RENDERER_KIND {
        chains.push(PlannedMaskChain {
            stages: path
                .iter()
                .map(|id| stage(placements[id]))
                .collect::<Result<_, _>>()?,
        });
    } else {
        let successors = next
            .get(&placement.placement_id)
            .ok_or(MaskTopologyError::MissingTerminalRenderer)?;
        if successors.is_empty() {
            return Err(MaskTopologyError::MissingTerminalRenderer);
        }
        for id in successors {
            walk(
                placements.get(id).ok_or(MaskTopologyError::InvalidPlan)?,
                placements,
                next,
                path,
                chains,
            )?;
        }
    }
    path.pop();
    Ok(())
}

fn is_mask_stage(p: &PlannedGear) -> bool {
    matches!(p.kind_id.as_str(), PRESENTER_STAGE_KIND | RENDERER_KIND)
}

fn validate_stage(p: &PlannedGear) -> Result<(), MaskTopologyError> {
    if p.inputs.len() != 1 || p.inputs[0].value_kind.as_str() != PRESENTATION_VALUE_KIND {
        return Err(MaskTopologyError::InvalidStageContract);
    }
    let final_stage = p.kind_id.as_str() == RENDERER_KIND;
    if (!final_stage
        && (p.outputs.len() != 1 || p.outputs[0].value_kind.as_str() != PRESENTATION_VALUE_KIND))
        || (final_stage && p.outputs.len() != 1)
    {
        return Err(MaskTopologyError::IncompatibleType);
    }
    Ok(())
}

fn stage(p: &PlannedGear) -> Result<PlannedMaskStage, MaskTopologyError> {
    validate_stage(p)?;
    Ok(PlannedMaskStage {
        placement_id: p.placement_id.clone(),
        capability_id: p.capability_id.clone(),
        implementation_id: p.implementation_id.clone(),
        host_id: p.host_id.clone(),
        boot_id: p.boot_id.clone(),
        input_kind: p.inputs[0].value_kind.clone(),
        output_kind: p.outputs[0].value_kind.clone(),
        input_item_capacity: p.limits.max_queue_items,
        input_byte_capacity: p.limits.max_queue_bytes,
        resources: p.resources.clone(),
    })
}
