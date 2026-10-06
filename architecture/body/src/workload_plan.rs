//! One immutable body-wide Plan over the current exact plot workset.

use alloc::{format, string::String, vec::Vec};
use conduit_core::{verify_plan, ActivePlayId, BodyTimeRequirement, PlacementId, Plan, PlanId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{BodyId, BodyWorkset, ResidentPlot, Wake, WakeId, MAX_BODY_PLOTS};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlotPlan {
    pub plot: ResidentPlot,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlan {
    pub plan_id: PlanId,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub workload_revision: u64,
    pub workset: BodyWorkset,
    pub plots: Vec<BodyPlotPlan>,
    /// Exact Mask Plot realizations selected independently of authored plots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask_topologies: Vec<BodyMaskTopology>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_time_requirement: Option<BodyTimeRequirement>,
}

pub const MAX_BODY_MASK_TOPOLOGIES: usize = 16;
pub const MAX_BODY_MASK_CHAINS: usize = 8;
pub const MAX_BODY_MASK_STAGES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BodyFaceSelector {
    /// The exact resident Plot in focus, if any. A Face composed by the Body
    /// owner may focus a Plot without originating at one of its placements.
    pub plot: Option<ResidentPlot>,
    /// Only a Face emitted by a planned Plot gear names its actual placement.
    /// `None` means the authoritative owner composed the Face and supplies its
    /// exact revision at the declared Mask input, even if a Plot is in focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_placement_id: Option<PlacementId>,
}

/// One independently admitted linear chain. `plan` is an ordinary immutable
/// Plan over reviewed realization Plots; `stage_placement_ids` fixes its path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyMaskChainPlan {
    pub plan: Plan,
    pub stage_placement_ids: Vec<PlacementId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyMaskTopology {
    pub face: BodyFaceSelector,
    pub chains: Vec<BodyMaskChainPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlayIdentity {
    pub active_play_id: ActivePlayId,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub plan_id: PlanId,
    pub play_sequence: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyPlanError {
    EmptyWorkset,
    PlotCapacityExceeded,
    InvalidPlan,
    DuplicatePlot,
    MissingPlot,
    UnexpectedPlot,
    WrongBody,
    WrongWake,
    StaleWorkload,
    InvalidIdentity,
    MaskTopologyCapacityExceeded,
    MaskChainCapacityExceeded,
    InvalidMaskChain,
    DuplicateMaskTopology,
    InvalidBodyTimeRequirement,
}

impl BodyPlan {
    pub fn seal(wake: &Wake, plots: Vec<BodyPlotPlan>) -> Result<Self, BodyPlanError> {
        Self::seal_with_masks(wake, plots, Vec::new())
    }

    pub fn seal_with_masks(
        wake: &Wake,
        plots: Vec<BodyPlotPlan>,
        mask_topologies: Vec<BodyMaskTopology>,
    ) -> Result<Self, BodyPlanError> {
        Self::seal_with_requirements(wake, plots, mask_topologies, None)
    }

    pub fn seal_with_body_time(
        wake: &Wake,
        plots: Vec<BodyPlotPlan>,
        requirement: BodyTimeRequirement,
    ) -> Result<Self, BodyPlanError> {
        Self::seal_with_requirements(wake, plots, Vec::new(), Some(requirement))
    }

    fn seal_with_requirements(
        wake: &Wake,
        mut plots: Vec<BodyPlotPlan>,
        mut mask_topologies: Vec<BodyMaskTopology>,
        body_time_requirement: Option<BodyTimeRequirement>,
    ) -> Result<Self, BodyPlanError> {
        validate_body_time_requirement(&wake.body_id, body_time_requirement.as_ref())?;
        wake.workset
            .validate()
            .map_err(|_| BodyPlanError::InvalidIdentity)?;
        if wake.workset.is_empty() {
            return Err(BodyPlanError::EmptyWorkset);
        }
        if plots.len() > MAX_BODY_PLOTS {
            return Err(BodyPlanError::PlotCapacityExceeded);
        }
        plots.sort_by(|left, right| left.plot.cmp(&right.plot));
        if plots.windows(2).any(|pair| pair[0].plot >= pair[1].plot) {
            return Err(BodyPlanError::DuplicatePlot);
        }
        for partition in &plots {
            if !wake.workset.contains(&partition.plot) {
                return Err(BodyPlanError::UnexpectedPlot);
            }
            if !verify_plan(&partition.plan)
                || partition.plan.source_document_id != partition.plot.source_document_id
                || partition.plan.checked_plot_id != partition.plot.checked_plot_id
            {
                return Err(BodyPlanError::InvalidPlan);
            }
        }
        if wake.workset.plots().iter().any(|plot| {
            plots
                .binary_search_by(|value| value.plot.cmp(plot))
                .is_err()
        }) {
            return Err(BodyPlanError::MissingPlot);
        }
        validate_mask_topologies(&wake.workset, &plots, &mask_topologies)?;
        mask_topologies.sort_by(|left, right| left.face.cmp(&right.face));
        let plan_id = bind_body_plan(
            &wake.body_id,
            &wake.wake_id,
            wake.workload_revision,
            &plots,
            &mask_topologies,
            body_time_requirement.as_ref(),
        );
        Ok(Self {
            plan_id,
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            workload_revision: wake.workload_revision,
            workset: wake.workset.clone(),
            plots,
            mask_topologies,
            body_time_requirement,
        })
    }

    pub fn validate_for(&self, wake: &Wake) -> Result<(), BodyPlanError> {
        if self.body_id != wake.body_id {
            return Err(BodyPlanError::WrongBody);
        }
        if self.wake_id != wake.wake_id {
            return Err(BodyPlanError::WrongWake);
        }
        if self.workload_revision != wake.workload_revision || self.workset != wake.workset {
            return Err(BodyPlanError::StaleWorkload);
        }
        let resealed = Self::seal_with_requirements(
            wake,
            self.plots.clone(),
            self.mask_topologies.clone(),
            self.body_time_requirement.clone(),
        )?;
        if resealed.plan_id != self.plan_id {
            return Err(BodyPlanError::InvalidIdentity);
        }
        Ok(())
    }

    /// Revalidate the immutable body-wide seal without relying on ambient Wake
    /// state. Runtime consumers use this before trusting nested Plot Plans.
    pub fn verify_seal(&self) -> Result<(), BodyPlanError> {
        self.workset
            .validate()
            .map_err(|_| BodyPlanError::InvalidIdentity)?;
        if self.workset.is_empty() {
            return Err(BodyPlanError::EmptyWorkset);
        }
        if self.plots.len() > MAX_BODY_PLOTS {
            return Err(BodyPlanError::PlotCapacityExceeded);
        }
        if self
            .plots
            .windows(2)
            .any(|pair| pair[0].plot >= pair[1].plot)
        {
            return Err(BodyPlanError::DuplicatePlot);
        }
        for partition in &self.plots {
            if !self.workset.contains(&partition.plot)
                || !verify_plan(&partition.plan)
                || partition.plan.source_document_id != partition.plot.source_document_id
                || partition.plan.checked_plot_id != partition.plot.checked_plot_id
            {
                return Err(BodyPlanError::InvalidPlan);
            }
        }
        if self.workset.plots().iter().any(|plot| {
            self.plots
                .binary_search_by(|value| value.plot.cmp(plot))
                .is_err()
        }) {
            return Err(BodyPlanError::MissingPlot);
        }
        validate_mask_topologies(&self.workset, &self.plots, &self.mask_topologies)?;
        validate_body_time_requirement(&self.body_id, self.body_time_requirement.as_ref())?;
        if self
            .mask_topologies
            .windows(2)
            .any(|pair| pair[0].face >= pair[1].face)
            || self.plan_id
                != bind_body_plan(
                    &self.body_id,
                    &self.wake_id,
                    self.workload_revision,
                    &self.plots,
                    &self.mask_topologies,
                    self.body_time_requirement.as_ref(),
                )
        {
            return Err(BodyPlanError::InvalidIdentity);
        }
        Ok(())
    }
}

impl BodyPlayIdentity {
    pub fn bind(plan: &BodyPlan, play_sequence: u64) -> Self {
        Self {
            active_play_id: bind_body_play(
                &plan.body_id,
                &plan.wake_id,
                &plan.plan_id,
                play_sequence,
            ),
            body_id: plan.body_id.clone(),
            wake_id: plan.wake_id.clone(),
            plan_id: plan.plan_id.clone(),
            play_sequence,
        }
    }

    pub fn validate_for(&self, plan: &BodyPlan) -> bool {
        self.body_id == plan.body_id
            && self.wake_id == plan.wake_id
            && self.plan_id == plan.plan_id
            && self.active_play_id
                == bind_body_play(
                    &self.body_id,
                    &self.wake_id,
                    &self.plan_id,
                    self.play_sequence,
                )
    }
}

fn bind_body_plan(
    body_id: &BodyId,
    wake_id: &WakeId,
    workload_revision: u64,
    plots: &[BodyPlotPlan],
    mask_topologies: &[BodyMaskTopology],
    body_time_requirement: Option<&BodyTimeRequirement>,
) -> PlanId {
    let mut bytes = Vec::new();
    push(&mut bytes, "conduit.body/body-plan@3");
    push(&mut bytes, body_id.as_str());
    push(&mut bytes, wake_id.as_str());
    bytes.extend_from_slice(&workload_revision.to_le_bytes());
    bytes.extend_from_slice(&(plots.len() as u32).to_le_bytes());
    for plot in plots {
        push(&mut bytes, plot.plot.source_document_id.as_str());
        push(&mut bytes, plot.plot.checked_plot_id.as_str());
        push(&mut bytes, plot.plan.plan_id.as_str());
    }
    bytes.extend_from_slice(&(mask_topologies.len() as u32).to_le_bytes());
    for topology in mask_topologies {
        if let Some(plot) = &topology.face.plot {
            push(&mut bytes, "plot");
            push(&mut bytes, plot.source_document_id.as_str());
            push(&mut bytes, plot.checked_plot_id.as_str());
        } else {
            push(&mut bytes, "body");
        }
        match &topology.face.source_placement_id {
            Some(placement) => {
                push(&mut bytes, "plot-placement");
                push(&mut bytes, placement.as_str());
            }
            None => push(&mut bytes, "owner-composed"),
        }
        bytes.extend_from_slice(&(topology.chains.len() as u32).to_le_bytes());
        for chain in &topology.chains {
            push(&mut bytes, chain.plan.plan_id.as_str());
            bytes.extend_from_slice(&(chain.stage_placement_ids.len() as u32).to_le_bytes());
            for placement in &chain.stage_placement_ids {
                push(&mut bytes, placement.as_str());
            }
        }
    }
    if let Some(requirement) = body_time_requirement {
        push(&mut bytes, "body-time-requirement@1");
        push(&mut bytes, requirement.body_basis());
        bytes.extend_from_slice(&requirement.tolerance().ticks().to_le_bytes());
        push_temporal_scale(&mut bytes, requirement.tolerance().scale());
        bytes.extend_from_slice(&requirement.horizon().ticks().to_le_bytes());
        push_temporal_scale(&mut bytes, requirement.horizon().scale());
    }
    PlanId::from(digest_id("body-plan", &bytes))
}

fn validate_body_time_requirement(
    body_id: &BodyId,
    requirement: Option<&BodyTimeRequirement>,
) -> Result<(), BodyPlanError> {
    if requirement.is_some_and(|requirement| {
        !requirement.valid_basis() || requirement.body_basis() != body_id.as_str()
    }) {
        return Err(BodyPlanError::InvalidBodyTimeRequirement);
    }
    Ok(())
}

fn push_temporal_scale(bytes: &mut Vec<u8>, scale: conduit_core::TemporalScale) {
    let label = match scale {
        conduit_core::TemporalScale::Seconds => "seconds",
        conduit_core::TemporalScale::Milliseconds => "milliseconds",
        conduit_core::TemporalScale::Microseconds => "microseconds",
        conduit_core::TemporalScale::Nanoseconds => "nanoseconds",
    };
    push(bytes, label);
}

fn validate_mask_topologies(
    workset: &BodyWorkset,
    plots: &[BodyPlotPlan],
    topologies: &[BodyMaskTopology],
) -> Result<(), BodyPlanError> {
    if topologies.len() > MAX_BODY_MASK_TOPOLOGIES {
        return Err(BodyPlanError::MaskTopologyCapacityExceeded);
    }
    for (index, topology) in topologies.iter().enumerate() {
        let valid_source = match (&topology.face.plot, &topology.face.source_placement_id) {
            (None, None) => true,
            (Some(plot), None) => workset.contains(plot),
            (Some(plot), Some(source)) => {
                workset.contains(plot)
                    && plots.iter().any(|partition| {
                        &partition.plot == plot
                            && partition
                                .plan
                                .fragments
                                .iter()
                                .flat_map(|fragment| &fragment.placements)
                                .any(|placement| &placement.placement_id == source)
                    })
            }
            _ => false,
        };
        if !valid_source
            || topology.chains.is_empty()
            || topology.chains.len() > MAX_BODY_MASK_CHAINS
        {
            return Err(BodyPlanError::InvalidMaskChain);
        }
        if topologies[index + 1..]
            .iter()
            .any(|other| other.face == topology.face)
        {
            return Err(BodyPlanError::DuplicateMaskTopology);
        }
        for chain in &topology.chains {
            if !verify_plan(&chain.plan)
                || chain.stage_placement_ids.is_empty()
                || chain.stage_placement_ids.len() > MAX_BODY_MASK_STAGES
            {
                return Err(BodyPlanError::InvalidMaskChain);
            }
            let placements = chain
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .collect::<Vec<_>>();
            for (stage_index, stage_id) in chain.stage_placement_ids.iter().enumerate() {
                if chain.stage_placement_ids[..stage_index].contains(stage_id)
                    || placements
                        .iter()
                        .filter(|placement| &placement.placement_id == stage_id)
                        .count()
                        != 1
                {
                    return Err(BodyPlanError::InvalidMaskChain);
                }
                if let Some(next_id) = chain.stage_placement_ids.get(stage_index + 1) {
                    let connected = chain
                        .plan
                        .fragments
                        .iter()
                        .flat_map(|f| &f.connections)
                        .any(|cord| {
                            &cord.source_placement_id == stage_id
                                && &cord.sink_placement_id == next_id
                        });
                    if !connected {
                        return Err(BodyPlanError::InvalidMaskChain);
                    }
                }
            }
        }
    }
    Ok(())
}

fn bind_body_play(
    body_id: &BodyId,
    wake_id: &WakeId,
    plan_id: &PlanId,
    play_sequence: u64,
) -> ActivePlayId {
    let mut bytes = Vec::new();
    push(&mut bytes, "conduit.body/body-play@1");
    push(&mut bytes, body_id.as_str());
    push(&mut bytes, wake_id.as_str());
    push(&mut bytes, plan_id.as_str());
    bytes.extend_from_slice(&play_sequence.to_le_bytes());
    ActivePlayId::from(digest_id("body-play", &bytes))
}

fn push(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn digest_id(prefix: &str, bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("{prefix}/sha256:{hex}")
}
