//! Typed, finite control of the current body Mask realization.
//!
//! These records are a projection and a replanning request. They do not mutate
//! a plan, schedule a renderer, or grant authority.

use conduit_body::{BodyId, BodyPlan, BodyPlayIdentity};
use conduit_core::{
    verify_plan, ActivePlayId, ActivePlayIdentity, BootId, CapabilityId, HostId, ImplementationId,
    PlacementId, Plan, PlanId, SourceDocumentId,
};
use conduit_presentation::{
    Manifestation, ManifestationLifecycle, ManifestationSet, MaskTopologyAdmission, Presentation,
};
use serde::{Deserialize, Serialize};

pub const MASK_TOPOLOGY_SCHEMA: &str = "conduit.patchbay/mask-topology@1";
pub const MAX_MASK_CHAINS: usize = 4;
pub const MAX_MASK_STAGES_PER_CHAIN: usize = 4;
pub const MAX_MASK_CAPACITY: u32 = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskStage {
    pub stage_id: String,
    pub placement_id: PlacementId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub input_kind: String,
    pub output_kind: Option<String>,
    pub capacity_cost: u32,
    pub available: bool,
    pub authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskChain {
    pub chain_id: String,
    pub stages: Vec<MaskStage>,
    pub manifestation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskTopology {
    pub schema: String,
    pub body_id: BodyId,
    pub source_document_id: Option<SourceDocumentId>,
    pub presentation_id: String,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub chains: Vec<MaskChain>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskTopologyChange {
    Add(MaskChain),
    Remove {
        chain_id: String,
    },
    Replace {
        chain_id: String,
        replacement: MaskChain,
    },
    Reorder {
        chain_id: String,
        stage_ids: Vec<String>,
    },
    SetParallel {
        chains: Vec<MaskChain>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskTopologyRequest {
    pub body_id: BodyId,
    pub source_document_id: Option<SourceDocumentId>,
    pub presentation_id: String,
    pub basis_plan_id: PlanId,
    pub change: MaskTopologyChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskTopologyReplacement {
    pub prior: MaskTopology,
    pub current: MaskTopology,
    pub replaced_manifestations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskTopologyRefusal {
    InvalidTopology,
    StaleRequest,
    SupersededBodyTruth,
    UnknownChain,
    DuplicateChain,
    UnavailableMask,
    LostHost,
    IncompatibleType,
    ResourcePressure,
    AuthorityPolicy,
    Cycle,
    ChainLengthBound,
    ParallelChainBound,
    ReusedPlan,
    ReusedPlay,
}

impl core::fmt::Display for MaskTopologyRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Mask topology request refused: {self:?}")
    }
}
impl std::error::Error for MaskTopologyRefusal {}

impl MaskTopology {
    /// Project from the body-wide Plan selection and exact terminal receipts.
    pub fn from_body_plan_truth(
        presentation: &Presentation,
        body_plan: &BodyPlan,
        body_play: &BodyPlayIdentity,
        manifestations: &[Manifestation],
    ) -> Result<Self, MaskTopologyRefusal> {
        if !body_play.validate_for(body_plan)
            || presentation.basis.body_id.as_ref() != Some(&body_plan.body_id)
        {
            return Err(MaskTopologyRefusal::SupersededBodyTruth);
        }
        let selected = body_plan
            .mask_topologies
            .iter()
            .find(|topology| match &topology.face.form {
                Some(form) => {
                    presentation.basis.source_document_id.as_ref() == Some(&form.source_document_id)
                        && presentation.basis.checked_form_id.as_ref()
                            == Some(&form.checked_form_id)
                }
                None => {
                    presentation.basis.source_document_id.is_none()
                        && presentation.basis.checked_form_id.is_none()
                }
            })
            .ok_or(MaskTopologyRefusal::InvalidTopology)?;
        let mut chains = Vec::with_capacity(selected.chains.len());
        for (chain_index, selected_chain) in selected.chains.iter().enumerate() {
            let admission = MaskTopologyAdmission::from_plan(&selected_chain.plan)
                .map_err(map_admission_error)?;
            let admitted = admission
                .chains
                .iter()
                .find(|chain| {
                    chain
                        .stages
                        .iter()
                        .map(|stage| &stage.placement_id)
                        .eq(selected_chain.stage_placement_ids.iter())
                })
                .ok_or(MaskTopologyRefusal::InvalidTopology)?;
            let terminal = admitted
                .stages
                .last()
                .ok_or(MaskTopologyRefusal::InvalidTopology)?;
            let manifestation = manifestations
                .iter()
                .find(|value| {
                    value.plan_id == selected_chain.plan.plan_id
                        && value.placement_id == terminal.placement_id
                })
                .ok_or(MaskTopologyRefusal::UnavailableMask)?;
            manifestation
                .validate_against(presentation, &selected_chain.plan)
                .map_err(|_| MaskTopologyRefusal::SupersededBodyTruth)?;
            chains.push(chain_from_admission(chain_index, admitted, manifestation));
        }
        Self::new(
            body_plan.body_id.clone(),
            presentation.basis.source_document_id.clone(),
            presentation.identity.as_str().into(),
            body_plan.plan_id.clone(),
            body_play.active_play_id.clone(),
            chains,
        )
    }

    /// Project Patchbay control state from the sealed Plan topology and exact
    /// terminal Manifestations. No chain or placement fact is supplied by the
    /// renderer or by Patchbay.
    pub fn from_plan_truth(
        body_id: BodyId,
        presentation: &Presentation,
        plan: &Plan,
        active_play_id: ActivePlayId,
        manifestations: &ManifestationSet,
    ) -> Result<Self, MaskTopologyRefusal> {
        if presentation.basis.body_id.as_ref() != Some(&body_id)
            || presentation.basis.source_document_id.as_ref() != Some(&plan.source_document_id)
            || manifestations.presentation_id != presentation.identity
            || manifestations.presentation_revision != presentation.revision
        {
            return Err(MaskTopologyRefusal::SupersededBodyTruth);
        }
        let admission = MaskTopologyAdmission::from_plan(plan).map_err(map_admission_error)?;
        let chains = admission
            .chains
            .into_iter()
            .enumerate()
            .map(|(chain_index, chain)| {
                let terminal = chain
                    .stages
                    .last()
                    .ok_or(MaskTopologyRefusal::InvalidTopology)?;
                let manifestation = manifestations
                    .manifestations
                    .iter()
                    .find(|value| value.placement_id == terminal.placement_id)
                    .ok_or(MaskTopologyRefusal::UnavailableMask)?;
                let available = manifestation.lifecycle == ManifestationLifecycle::Available;
                let stage_count = chain.stages.len();
                let stages = chain
                    .stages
                    .into_iter()
                    .enumerate()
                    .map(|(stage_index, stage)| MaskStage {
                        stage_id: stage.placement_id.as_str().into(),
                        placement_id: stage.placement_id,
                        capability_id: stage.capability_id,
                        implementation_id: stage.implementation_id,
                        host_id: stage.host_id,
                        boot_id: stage.boot_id,
                        input_kind: stage.input_kind.as_str().into(),
                        output_kind: (stage_index + 1 < stage_count)
                            .then(|| stage.output_kind.as_str().into()),
                        capacity_cost: u32::from(stage.input_item_capacity),
                        available,
                        authorized: true,
                    })
                    .collect();
                Ok(MaskChain {
                    chain_id: format!("chain/{chain_index}"),
                    stages,
                    manifestation_id: manifestation.manifestation_id.as_str().into(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            body_id,
            Some(plan.source_document_id.clone()),
            presentation.identity.as_str().into(),
            plan.plan_id.clone(),
            active_play_id,
            chains,
        )
    }

    pub fn new(
        body_id: BodyId,
        source_document_id: Option<SourceDocumentId>,
        presentation_id: String,
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        chains: Vec<MaskChain>,
    ) -> Result<Self, MaskTopologyRefusal> {
        let value = Self {
            schema: MASK_TOPOLOGY_SCHEMA.into(),
            body_id,
            source_document_id,
            presentation_id,
            plan_id,
            active_play_id,
            chains,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MaskTopologyRefusal> {
        if self.schema != MASK_TOPOLOGY_SCHEMA || self.presentation_id.is_empty() {
            return Err(MaskTopologyRefusal::InvalidTopology);
        }
        validate_chains(&self.chains)
    }

    /// On refusal `self` remains the live realization. Success describes a
    /// fresh immutable Plan/Play chosen by the ordinary planning owner.
    pub(crate) fn request_replacement(
        &self,
        request: MaskTopologyRequest,
        replacement_plan_id: PlanId,
        replacement_play_id: ActivePlayId,
    ) -> Result<MaskTopologyReplacement, MaskTopologyRefusal> {
        self.validate()?;
        if request.body_id != self.body_id
            || request.source_document_id != self.source_document_id
            || request.presentation_id != self.presentation_id
        {
            return Err(MaskTopologyRefusal::SupersededBodyTruth);
        }
        if request.basis_plan_id != self.plan_id {
            return Err(MaskTopologyRefusal::StaleRequest);
        }
        if replacement_plan_id == self.plan_id {
            return Err(MaskTopologyRefusal::ReusedPlan);
        }
        if replacement_play_id == self.active_play_id {
            return Err(MaskTopologyRefusal::ReusedPlay);
        }
        let mut chains = self.chains.clone();
        match request.change {
            MaskTopologyChange::Add(chain) => {
                if chains.iter().any(|value| value.chain_id == chain.chain_id) {
                    return Err(MaskTopologyRefusal::DuplicateChain);
                }
                chains.push(chain);
            }
            MaskTopologyChange::Remove { chain_id } => {
                let index = chain_index(&chains, &chain_id)?;
                chains.remove(index);
            }
            MaskTopologyChange::Replace {
                chain_id,
                replacement,
            } => {
                let index = chain_index(&chains, &chain_id)?;
                if replacement.chain_id != chain_id
                    && chains
                        .iter()
                        .any(|value| value.chain_id == replacement.chain_id)
                {
                    return Err(MaskTopologyRefusal::DuplicateChain);
                }
                chains[index] = replacement;
            }
            MaskTopologyChange::Reorder {
                chain_id,
                stage_ids,
            } => {
                let index = chain_index(&chains, &chain_id)?;
                let old = chains[index].stages.clone();
                if stage_ids.len() != old.len() {
                    return Err(MaskTopologyRefusal::IncompatibleType);
                }
                let mut reordered = Vec::with_capacity(old.len());
                for id in stage_ids {
                    let stage = old
                        .iter()
                        .find(|stage| stage.stage_id == id)
                        .ok_or(MaskTopologyRefusal::IncompatibleType)?;
                    if reordered
                        .iter()
                        .any(|value: &MaskStage| value.stage_id == id)
                    {
                        return Err(MaskTopologyRefusal::Cycle);
                    }
                    reordered.push(stage.clone());
                }
                chains[index].stages = reordered;
            }
            MaskTopologyChange::SetParallel {
                chains: replacement,
            } => chains = replacement,
        }
        validate_chains(&chains)?;
        let replaced_manifestations = self
            .chains
            .iter()
            .filter(|old| {
                !chains
                    .iter()
                    .any(|new| new.manifestation_id == old.manifestation_id)
            })
            .map(|chain| chain.manifestation_id.clone())
            .collect();
        let current = Self::new(
            self.body_id.clone(),
            self.source_document_id.clone(),
            self.presentation_id.clone(),
            replacement_plan_id,
            replacement_play_id,
            chains,
        )?;
        Ok(MaskTopologyReplacement {
            prior: self.clone(),
            current,
            replaced_manifestations,
        })
    }

    /// Accept a topology change only after the ordinary planner has produced
    /// a verified replacement Plan containing every requested exact stage.
    pub fn request_replacement_from_plan(
        &self,
        request: MaskTopologyRequest,
        replacement_plan: &Plan,
        replacement_play: ActivePlayIdentity,
    ) -> Result<MaskTopologyReplacement, MaskTopologyRefusal> {
        if !verify_plan(replacement_plan)
            || Some(&replacement_plan.source_document_id) != self.source_document_id.as_ref()
            || replacement_play.plan_id != replacement_plan.plan_id
            || replacement_play.active_play_id == self.active_play_id
        {
            return Err(MaskTopologyRefusal::InvalidTopology);
        }
        let result = self.request_replacement(
            request,
            replacement_plan.plan_id.clone(),
            replacement_play.active_play_id,
        )?;
        validate_chains_against_plan(&result.current.chains, replacement_plan)?;
        Ok(result)
    }
}

fn map_admission_error(error: conduit_presentation::MaskTopologyError) -> MaskTopologyRefusal {
    match error {
        conduit_presentation::MaskTopologyError::IncompatibleType
        | conduit_presentation::MaskTopologyError::InvalidStageContract => {
            MaskTopologyRefusal::IncompatibleType
        }
        conduit_presentation::MaskTopologyError::Cycle => MaskTopologyRefusal::Cycle,
        conduit_presentation::MaskTopologyError::ChainLengthBound => {
            MaskTopologyRefusal::ChainLengthBound
        }
        conduit_presentation::MaskTopologyError::ParallelChainBound => {
            MaskTopologyRefusal::ParallelChainBound
        }
        _ => MaskTopologyRefusal::InvalidTopology,
    }
}

fn chain_from_admission(
    chain_index: usize,
    chain: &conduit_presentation::PlannedMaskChain,
    manifestation: &Manifestation,
) -> MaskChain {
    let available = manifestation.lifecycle == ManifestationLifecycle::Available;
    let stage_count = chain.stages.len();
    MaskChain {
        chain_id: format!("chain/{chain_index}"),
        stages: chain
            .stages
            .iter()
            .enumerate()
            .map(|(stage_index, stage)| MaskStage {
                stage_id: stage.placement_id.as_str().into(),
                placement_id: stage.placement_id.clone(),
                capability_id: stage.capability_id.clone(),
                implementation_id: stage.implementation_id.clone(),
                host_id: stage.host_id.clone(),
                boot_id: stage.boot_id.clone(),
                input_kind: stage.input_kind.as_str().into(),
                output_kind: (stage_index + 1 < stage_count)
                    .then(|| stage.output_kind.as_str().into()),
                capacity_cost: u32::from(stage.input_item_capacity),
                available,
                authorized: true,
            })
            .collect(),
        manifestation_id: manifestation.manifestation_id.as_str().into(),
    }
}

fn chain_index(chains: &[MaskChain], id: &str) -> Result<usize, MaskTopologyRefusal> {
    chains
        .iter()
        .position(|chain| chain.chain_id == id)
        .ok_or(MaskTopologyRefusal::UnknownChain)
}

pub(crate) fn validate_chains(chains: &[MaskChain]) -> Result<(), MaskTopologyRefusal> {
    if chains.is_empty() || chains.len() > MAX_MASK_CHAINS {
        return Err(MaskTopologyRefusal::ParallelChainBound);
    }
    let mut capacity = 0_u32;
    for (index, chain) in chains.iter().enumerate() {
        if chain.chain_id.is_empty() || chain.manifestation_id.is_empty() || chain.stages.is_empty()
        {
            return Err(MaskTopologyRefusal::InvalidTopology);
        }
        if chains[index + 1..]
            .iter()
            .any(|candidate| candidate.chain_id == chain.chain_id)
        {
            return Err(MaskTopologyRefusal::DuplicateChain);
        }
        if chain.stages.len() > MAX_MASK_STAGES_PER_CHAIN {
            return Err(MaskTopologyRefusal::ChainLengthBound);
        }
        for (stage_index, stage) in chain.stages.iter().enumerate() {
            if stage.stage_id.is_empty() || stage.input_kind.is_empty() {
                return Err(MaskTopologyRefusal::InvalidTopology);
            }
            if chain.stages[stage_index + 1..]
                .iter()
                .any(|candidate| candidate.stage_id == stage.stage_id)
            {
                return Err(MaskTopologyRefusal::Cycle);
            }
            if !stage.available {
                return Err(if stage_index == 0 {
                    MaskTopologyRefusal::UnavailableMask
                } else {
                    MaskTopologyRefusal::LostHost
                });
            }
            if !stage.authorized {
                return Err(MaskTopologyRefusal::AuthorityPolicy);
            }
            capacity = capacity
                .checked_add(stage.capacity_cost)
                .ok_or(MaskTopologyRefusal::ResourcePressure)?;
            if let Some(next) = chain.stages.get(stage_index + 1) {
                if stage.output_kind.as_deref() != Some(next.input_kind.as_str()) {
                    return Err(MaskTopologyRefusal::IncompatibleType);
                }
            } else if stage.output_kind.is_some() {
                return Err(MaskTopologyRefusal::IncompatibleType);
            }
        }
    }
    if capacity > MAX_MASK_CAPACITY {
        return Err(MaskTopologyRefusal::ResourcePressure);
    }
    Ok(())
}

fn validate_chains_against_plan(
    chains: &[MaskChain],
    plan: &Plan,
) -> Result<(), MaskTopologyRefusal> {
    for stage in chains.iter().flat_map(|chain| chain.stages.iter()) {
        let mut placements = plan
            .fragments
            .iter()
            .flat_map(|fragment| fragment.placements.iter())
            .filter(|placement| placement.placement_id == stage.placement_id);
        let placement = placements
            .next()
            .ok_or(MaskTopologyRefusal::UnavailableMask)?;
        if placements.next().is_some()
            || placement.capability_id != stage.capability_id
            || placement.implementation_id != stage.implementation_id
            || placement.host_id != stage.host_id
            || placement.boot_id != stage.boot_id
        {
            return Err(MaskTopologyRefusal::SupersededBodyTruth);
        }
    }
    Ok(())
}
