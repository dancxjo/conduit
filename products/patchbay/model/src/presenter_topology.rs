//! Typed, finite control of the current Body Presenter realization.
//!
//! These records are a projection and a replanning request. They do not mutate
//! a Plan, schedule a renderer, or grant authority.

use conduit_body::{BodyId, BodyPlan, BodyPlayIdentity};
use conduit_core::{
    verify_plan, ActivePlayId, ActivePlayIdentity, BootId, CapabilityId, HostId, ImplementationId,
    PlacementId, Plan, PlanId, SourceDocumentId,
};
use conduit_presentation::{
    Manifestation, ManifestationLifecycle, ManifestationSet, Presentation, PresentationAction,
    PresentationActionAvailability, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
    PresenterTopologyAdmission,
};
use serde::{Deserialize, Serialize};

pub const PRESENTER_TOPOLOGY_SCHEMA: &str = "conduit.patchbay/presenter-topology@1";
pub const MAX_PRESENTER_CHAINS: usize = 4;
pub const MAX_PRESENTER_STAGES_PER_CHAIN: usize = 4;
pub const MAX_PRESENTER_CAPACITY: u32 = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenterStage {
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
pub struct PresenterChain {
    pub chain_id: String,
    pub stages: Vec<PresenterStage>,
    pub manifestation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenterTopology {
    pub schema: String,
    pub body_id: BodyId,
    pub source_document_id: Option<SourceDocumentId>,
    pub presentation_id: String,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub chains: Vec<PresenterChain>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenterTopologyChange {
    Add(PresenterChain),
    Remove {
        chain_id: String,
    },
    Replace {
        chain_id: String,
        replacement: PresenterChain,
    },
    Reorder {
        chain_id: String,
        stage_ids: Vec<String>,
    },
    SetParallel {
        chains: Vec<PresenterChain>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenterTopologyRequest {
    pub body_id: BodyId,
    pub source_document_id: Option<SourceDocumentId>,
    pub presentation_id: String,
    pub basis_plan_id: PlanId,
    pub change: PresenterTopologyChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenterTopologyReplacement {
    pub prior: PresenterTopology,
    pub current: PresenterTopology,
    pub replaced_manifestations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenterTopologyRefusal {
    InvalidTopology,
    StaleRequest,
    SupersededBodyTruth,
    UnknownChain,
    DuplicateChain,
    UnavailablePresenter,
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

impl core::fmt::Display for PresenterTopologyRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Presenter topology request refused: {self:?}")
    }
}
impl std::error::Error for PresenterTopologyRefusal {}

impl PresenterTopology {
    /// Project from the Body-wide Plan selection and exact terminal receipts.
    pub fn from_body_plan_truth(
        presentation: &Presentation,
        body_plan: &BodyPlan,
        body_play: &BodyPlayIdentity,
        manifestations: &[Manifestation],
    ) -> Result<Self, PresenterTopologyRefusal> {
        if !body_play.validate_for(body_plan)
            || presentation.basis.body_id.as_ref() != Some(&body_plan.body_id)
        {
            return Err(PresenterTopologyRefusal::SupersededBodyTruth);
        }
        let selected = body_plan
            .presenter_topologies
            .iter()
            .find(|topology| match &topology.presentation.form {
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
            .ok_or(PresenterTopologyRefusal::InvalidTopology)?;
        let mut chains = Vec::with_capacity(selected.chains.len());
        for (chain_index, selected_chain) in selected.chains.iter().enumerate() {
            let admission = PresenterTopologyAdmission::from_plan(&selected_chain.plan)
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
                .ok_or(PresenterTopologyRefusal::InvalidTopology)?;
            let terminal = admitted
                .stages
                .last()
                .ok_or(PresenterTopologyRefusal::InvalidTopology)?;
            let manifestation = manifestations
                .iter()
                .find(|value| {
                    value.plan_id == selected_chain.plan.plan_id
                        && value.placement_id == terminal.placement_id
                })
                .ok_or(PresenterTopologyRefusal::UnavailablePresenter)?;
            manifestation
                .validate_against(presentation, &selected_chain.plan)
                .map_err(|_| PresenterTopologyRefusal::SupersededBodyTruth)?;
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
    ) -> Result<Self, PresenterTopologyRefusal> {
        if presentation.basis.body_id.as_ref() != Some(&body_id)
            || presentation.basis.source_document_id.as_ref() != Some(&plan.source_document_id)
            || manifestations.presentation_id != presentation.identity
            || manifestations.presentation_revision != presentation.revision
        {
            return Err(PresenterTopologyRefusal::SupersededBodyTruth);
        }
        let admission = PresenterTopologyAdmission::from_plan(plan).map_err(map_admission_error)?;
        let chains = admission
            .chains
            .into_iter()
            .enumerate()
            .map(|(chain_index, chain)| {
                let terminal = chain
                    .stages
                    .last()
                    .ok_or(PresenterTopologyRefusal::InvalidTopology)?;
                let manifestation = manifestations
                    .manifestations
                    .iter()
                    .find(|value| value.placement_id == terminal.placement_id)
                    .ok_or(PresenterTopologyRefusal::UnavailablePresenter)?;
                let available = manifestation.lifecycle == ManifestationLifecycle::Available;
                let stage_count = chain.stages.len();
                let stages = chain
                    .stages
                    .into_iter()
                    .enumerate()
                    .map(|(stage_index, stage)| PresenterStage {
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
                Ok(PresenterChain {
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
        chains: Vec<PresenterChain>,
    ) -> Result<Self, PresenterTopologyRefusal> {
        let value = Self {
            schema: PRESENTER_TOPOLOGY_SCHEMA.into(),
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

    pub fn validate(&self) -> Result<(), PresenterTopologyRefusal> {
        if self.schema != PRESENTER_TOPOLOGY_SCHEMA || self.presentation_id.is_empty() {
            return Err(PresenterTopologyRefusal::InvalidTopology);
        }
        validate_chains(&self.chains)
    }

    /// On refusal `self` remains the live realization. Success describes a
    /// fresh immutable Plan/Play chosen by the ordinary planning owner.
    pub(crate) fn request_replacement(
        &self,
        request: PresenterTopologyRequest,
        replacement_plan_id: PlanId,
        replacement_play_id: ActivePlayId,
    ) -> Result<PresenterTopologyReplacement, PresenterTopologyRefusal> {
        self.validate()?;
        if request.body_id != self.body_id
            || request.source_document_id != self.source_document_id
            || request.presentation_id != self.presentation_id
        {
            return Err(PresenterTopologyRefusal::SupersededBodyTruth);
        }
        if request.basis_plan_id != self.plan_id {
            return Err(PresenterTopologyRefusal::StaleRequest);
        }
        if replacement_plan_id == self.plan_id {
            return Err(PresenterTopologyRefusal::ReusedPlan);
        }
        if replacement_play_id == self.active_play_id {
            return Err(PresenterTopologyRefusal::ReusedPlay);
        }
        let mut chains = self.chains.clone();
        match request.change {
            PresenterTopologyChange::Add(chain) => {
                if chains.iter().any(|value| value.chain_id == chain.chain_id) {
                    return Err(PresenterTopologyRefusal::DuplicateChain);
                }
                chains.push(chain);
            }
            PresenterTopologyChange::Remove { chain_id } => {
                let index = chain_index(&chains, &chain_id)?;
                chains.remove(index);
            }
            PresenterTopologyChange::Replace {
                chain_id,
                replacement,
            } => {
                let index = chain_index(&chains, &chain_id)?;
                if replacement.chain_id != chain_id
                    && chains
                        .iter()
                        .any(|value| value.chain_id == replacement.chain_id)
                {
                    return Err(PresenterTopologyRefusal::DuplicateChain);
                }
                chains[index] = replacement;
            }
            PresenterTopologyChange::Reorder {
                chain_id,
                stage_ids,
            } => {
                let index = chain_index(&chains, &chain_id)?;
                let old = chains[index].stages.clone();
                if stage_ids.len() != old.len() {
                    return Err(PresenterTopologyRefusal::IncompatibleType);
                }
                let mut reordered = Vec::with_capacity(old.len());
                for id in stage_ids {
                    let stage = old
                        .iter()
                        .find(|stage| stage.stage_id == id)
                        .ok_or(PresenterTopologyRefusal::IncompatibleType)?;
                    if reordered
                        .iter()
                        .any(|value: &PresenterStage| value.stage_id == id)
                    {
                        return Err(PresenterTopologyRefusal::Cycle);
                    }
                    reordered.push(stage.clone());
                }
                chains[index].stages = reordered;
            }
            PresenterTopologyChange::SetParallel {
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
        Ok(PresenterTopologyReplacement {
            prior: self.clone(),
            current,
            replaced_manifestations,
        })
    }

    /// Accept a topology change only after the ordinary planner has produced
    /// a verified replacement Plan containing every requested exact stage.
    pub fn request_replacement_from_plan(
        &self,
        request: PresenterTopologyRequest,
        replacement_plan: &Plan,
        replacement_play: ActivePlayIdentity,
    ) -> Result<PresenterTopologyReplacement, PresenterTopologyRefusal> {
        if !verify_plan(replacement_plan)
            || Some(&replacement_plan.source_document_id) != self.source_document_id.as_ref()
            || replacement_play.plan_id != replacement_plan.plan_id
            || replacement_play.active_play_id == self.active_play_id
        {
            return Err(PresenterTopologyRefusal::InvalidTopology);
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

fn map_admission_error(
    error: conduit_presentation::PresenterTopologyError,
) -> PresenterTopologyRefusal {
    match error {
        conduit_presentation::PresenterTopologyError::IncompatibleType
        | conduit_presentation::PresenterTopologyError::InvalidStageContract => {
            PresenterTopologyRefusal::IncompatibleType
        }
        conduit_presentation::PresenterTopologyError::Cycle => PresenterTopologyRefusal::Cycle,
        conduit_presentation::PresenterTopologyError::ChainLengthBound => {
            PresenterTopologyRefusal::ChainLengthBound
        }
        conduit_presentation::PresenterTopologyError::ParallelChainBound => {
            PresenterTopologyRefusal::ParallelChainBound
        }
        _ => PresenterTopologyRefusal::InvalidTopology,
    }
}

fn chain_from_admission(
    chain_index: usize,
    chain: &conduit_presentation::PlannedPresenterChain,
    manifestation: &Manifestation,
) -> PresenterChain {
    let available = manifestation.lifecycle == ManifestationLifecycle::Available;
    let stage_count = chain.stages.len();
    PresenterChain {
        chain_id: format!("chain/{chain_index}"),
        stages: chain
            .stages
            .iter()
            .enumerate()
            .map(|(stage_index, stage)| PresenterStage {
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

fn chain_index(chains: &[PresenterChain], id: &str) -> Result<usize, PresenterTopologyRefusal> {
    chains
        .iter()
        .position(|chain| chain.chain_id == id)
        .ok_or(PresenterTopologyRefusal::UnknownChain)
}

pub(crate) fn validate_chains(chains: &[PresenterChain]) -> Result<(), PresenterTopologyRefusal> {
    if chains.is_empty() || chains.len() > MAX_PRESENTER_CHAINS {
        return Err(PresenterTopologyRefusal::ParallelChainBound);
    }
    let mut capacity = 0_u32;
    for (index, chain) in chains.iter().enumerate() {
        if chain.chain_id.is_empty() || chain.manifestation_id.is_empty() || chain.stages.is_empty()
        {
            return Err(PresenterTopologyRefusal::InvalidTopology);
        }
        if chains[index + 1..]
            .iter()
            .any(|candidate| candidate.chain_id == chain.chain_id)
        {
            return Err(PresenterTopologyRefusal::DuplicateChain);
        }
        if chain.stages.len() > MAX_PRESENTER_STAGES_PER_CHAIN {
            return Err(PresenterTopologyRefusal::ChainLengthBound);
        }
        for (stage_index, stage) in chain.stages.iter().enumerate() {
            if stage.stage_id.is_empty() || stage.input_kind.is_empty() {
                return Err(PresenterTopologyRefusal::InvalidTopology);
            }
            if chain.stages[stage_index + 1..]
                .iter()
                .any(|candidate| candidate.stage_id == stage.stage_id)
            {
                return Err(PresenterTopologyRefusal::Cycle);
            }
            if !stage.available {
                return Err(if stage_index == 0 {
                    PresenterTopologyRefusal::UnavailablePresenter
                } else {
                    PresenterTopologyRefusal::LostHost
                });
            }
            if !stage.authorized {
                return Err(PresenterTopologyRefusal::AuthorityPolicy);
            }
            capacity = capacity
                .checked_add(stage.capacity_cost)
                .ok_or(PresenterTopologyRefusal::ResourcePressure)?;
            if let Some(next) = chain.stages.get(stage_index + 1) {
                if stage.output_kind.as_deref() != Some(next.input_kind.as_str()) {
                    return Err(PresenterTopologyRefusal::IncompatibleType);
                }
            } else if stage.output_kind.is_some() {
                return Err(PresenterTopologyRefusal::IncompatibleType);
            }
        }
    }
    if capacity > MAX_PRESENTER_CAPACITY {
        return Err(PresenterTopologyRefusal::ResourcePressure);
    }
    Ok(())
}

fn validate_chains_against_plan(
    chains: &[PresenterChain],
    plan: &Plan,
) -> Result<(), PresenterTopologyRefusal> {
    for stage in chains.iter().flat_map(|chain| chain.stages.iter()) {
        let mut placements = plan
            .fragments
            .iter()
            .flat_map(|fragment| fragment.placements.iter())
            .filter(|placement| placement.placement_id == stage.placement_id);
        let placement = placements
            .next()
            .ok_or(PresenterTopologyRefusal::UnavailablePresenter)?;
        if placements.next().is_some()
            || placement.capability_id != stage.capability_id
            || placement.implementation_id != stage.implementation_id
            || placement.host_id != stage.host_id
            || placement.boot_id != stage.boot_id
        {
            return Err(PresenterTopologyRefusal::SupersededBodyTruth);
        }
    }
    Ok(())
}

/// Add exact Presenter-chain truth and typed controls to the portable
/// Presentation consumed by both browser and native Patchbay renderers.
pub fn project_presenter_topology(
    base: &Presentation,
    topology: &PresenterTopology,
) -> Result<Presentation, PresenterTopologyRefusal> {
    topology.validate()?;
    if base.basis.body_id.as_ref() != Some(&topology.body_id)
        || base.basis.source_document_id != topology.source_document_id
        || base.identity.as_str() != topology.presentation_id
    {
        return Err(PresenterTopologyRefusal::StaleRequest);
    }
    let (mut subjects, mut relationships, mut properties, mut text, mut actions, mut disclosures) = (
        base.subjects.clone(),
        base.relationships.clone(),
        base.properties.clone(),
        base.text.clone(),
        base.actions.clone(),
        base.disclosures.clone(),
    );
    let root = format!("presenter-topology/{}", topology.presentation_id);
    subjects.push(PresentationSubject {
        identity: root.clone(),
        role: PresentationRole::Plan,
        label: "Presenter topology".into(),
        accessibility_name: "Exact current Presenter chains from immutable Plan truth".into(),
    });
    properties.push(PresentationProperty {
        subject: root.clone(),
        name: "schema".into(),
        value: PresentationPropertyValue::Text(PRESENTER_TOPOLOGY_SCHEMA.into()),
    });
    properties.push(PresentationProperty {
        subject: root.clone(),
        name: "parallel-chain-count".into(),
        value: PresentationPropertyValue::Count(topology.chains.len() as u64),
    });
    disclosures.push(PresentationDisclosure {
        subject: root.clone(),
        level: PresentationDisclosureLevel::Primary,
    });
    actions.push(topology_action(&root, "add", "Add Presenter"));
    actions.push(topology_action(
        &root,
        "toggle-parallel",
        "Toggle parallel chains",
    ));
    for (chain_order, chain) in topology.chains.iter().enumerate() {
        let chain_subject = format!("{root}/chain/{}", chain.chain_id);
        subjects.push(PresentationSubject {
            identity: chain_subject.clone(),
            role: PresentationRole::Manifestation,
            label: format!("Presenter chain {}", chain.chain_id),
            accessibility_name: format!(
                "Presenter chain {} in order {}",
                chain.chain_id, chain_order
            ),
        });
        relationships.push(PresentationRelationship {
            source: root.clone(),
            target: chain_subject.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        properties.push(PresentationProperty {
            subject: chain_subject.clone(),
            name: "manifestation".into(),
            value: PresentationPropertyValue::Identity(chain.manifestation_id.clone()),
        });
        actions.push(topology_action(
            &chain_subject,
            "remove",
            "Remove Presenter chain",
        ));
        actions.push(topology_action(
            &chain_subject,
            "replace",
            "Replace Presenter",
        ));
        actions.push(topology_action(
            &chain_subject,
            "reorder",
            "Reorder Presenter stages",
        ));
        for (stage_order, stage) in chain.stages.iter().enumerate() {
            let stage_subject = format!("{chain_subject}/stage/{}", stage.stage_id);
            subjects.push(PresentationSubject {
                identity: stage_subject.clone(),
                role: PresentationRole::Capability,
                label: stage.implementation_id.as_str().into(),
                accessibility_name: format!(
                    "Presenter stage {} on Host {} Boot {}",
                    stage_order,
                    stage.host_id.as_str(),
                    stage.boot_id.as_str()
                ),
            });
            relationships.push(PresentationRelationship {
                source: chain_subject.clone(),
                target: stage_subject.clone(),
                kind: PresentationRelationshipKind::Contains,
            });
            for (name, value) in [
                ("placement", stage.placement_id.as_str()),
                ("capability", stage.capability_id.as_str()),
                ("implementation", stage.implementation_id.as_str()),
                ("host", stage.host_id.as_str()),
                ("boot", stage.boot_id.as_str()),
                ("input-kind", stage.input_kind.as_str()),
            ] {
                properties.push(PresentationProperty {
                    subject: stage_subject.clone(),
                    name: name.into(),
                    value: PresentationPropertyValue::Text(value.into()),
                });
            }
            properties.push(PresentationProperty {
                subject: stage_subject.clone(),
                name: "capacity-cost".into(),
                value: PresentationPropertyValue::Count(stage.capacity_cost.into()),
            });
            text.push(PresentationText {
                subject: stage_subject,
                text: format!(
                    "stage={} implementation={} host={} boot={} capacity={}",
                    stage_order,
                    stage.implementation_id.as_str(),
                    stage.host_id.as_str(),
                    stage.boot_id.as_str(),
                    stage.capacity_cost
                ),
            });
        }
    }
    Presentation::new_with_semantics(
        base.revision,
        base.basis.clone(),
        subjects,
        relationships,
        properties,
        text,
        actions,
        disclosures,
    )
    .map_err(|_| PresenterTopologyRefusal::InvalidTopology)
}

fn topology_action(target: &str, operation: &str, label: &str) -> PresentationAction {
    PresentationAction {
        identity: format!("action/presenter-topology/{operation}/{target}"),
        intent: format!("conduit.intent/presenter-topology-{operation}@1"),
        target: target.into(),
        label: label.into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: PresentationActionAvailability::Available,
    }
}
