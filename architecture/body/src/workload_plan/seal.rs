//! Immutable local evidence of a verified flat Body Plan. This is not execution
//! material, a serialized Plan representation, or resource/Play authority.
use super::{BodyFaceSelector, BodyPlan, BodyPlanError, Wake};
use crate::{BodyId, BodyWorkset, WakeId};
use alloc::vec::Vec;
use conduit_core::{
    BodyTimeRequirement, BootId, FragmentId, HostId, OfferGeneration, PlacementId, Plan, PlanId,
    PlotIdentity,
};

mod verified;

mod private {
    pub trait Sealed {}
}

/// Sealed lifecycle input: an inspectable rich Plan or locally retained evidence
/// of its immutable validation. External implementations cannot forge this seam.
///
/// ```compile_fail
/// #[derive(Clone, Debug, PartialEq, Eq)]
/// struct Forged;
/// impl conduit_body::BodyPlanEvidence for Forged {
///     fn body_id(&self) -> &conduit_body::BodyId { unimplemented!() }
///     fn wake_id(&self) -> &conduit_body::WakeId { unimplemented!() }
///     fn plan_id(&self) -> &conduit_core::PlanId { unimplemented!() }
///     fn workset(&self) -> &conduit_body::BodyWorkset { unimplemented!() }
///     fn validate_for(&self, _: &conduit_body::Wake) -> Result<(), conduit_body::BodyPlanError> { Ok(()) }
/// }
/// ```
pub trait BodyPlanEvidence: private::Sealed + Clone + core::fmt::Debug + PartialEq + Eq {
    fn body_id(&self) -> &BodyId;
    fn wake_id(&self) -> &WakeId;
    fn plan_id(&self) -> &PlanId;
    fn workset(&self) -> &BodyWorkset;
    fn validate_for(&self, wake: &Wake) -> Result<(), BodyPlanError>;
}
impl private::Sealed for BodyPlan {}
impl BodyPlanEvidence for BodyPlan {
    fn body_id(&self) -> &BodyId {
        &self.body_id
    }
    fn wake_id(&self) -> &WakeId {
        &self.wake_id
    }
    fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    fn workset(&self) -> &BodyWorkset {
        &self.workset
    }
    fn validate_for(&self, wake: &Wake) -> Result<(), BodyPlanError> {
        BodyPlan::validate_for(self, wake)
    }
}

pub const MAX_BODY_SEAL_PLANS: usize = 32;
pub const MAX_BODY_SEAL_FRAGMENTS: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyPlanSealRefusal {
    Invalid(BodyPlanError),
    Capacity,
    NestedExecution,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyFragmentSeal {
    fragment_id: FragmentId,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
}
impl BodyFragmentSeal {
    pub fn fragment_id(&self) -> &FragmentId {
        &self.fragment_id
    }
    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }
    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    pub fn offer_generation(&self) -> OfferGeneration {
        self.offer_generation
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyPlotPlanSeal {
    identity: PlotIdentity,
    plan_id: PlanId,
    fragments: Vec<BodyFragmentSeal>,
}
impl BodyPlotPlanSeal {
    pub fn identity(&self) -> &PlotIdentity {
        &self.identity
    }
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    pub fn fragments(&self) -> &[BodyFragmentSeal] {
        &self.fragments
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyMaskChainSeal {
    plan: BodyPlotPlanSeal,
    stages: Vec<PlacementId>,
}
impl BodyMaskChainSeal {
    pub fn plan(&self) -> &BodyPlotPlanSeal {
        &self.plan
    }
    pub fn stages(&self) -> &[PlacementId] {
        &self.stages
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyMaskTopologySeal {
    face: BodyFaceSelector,
    chains: Vec<BodyMaskChainSeal>,
}
impl BodyMaskTopologySeal {
    pub fn face(&self) -> &BodyFaceSelector {
        &self.face
    }
    pub fn chains(&self) -> &[BodyMaskChainSeal] {
        &self.chains
    }
}

/// Non-deserializable, privately owned validation evidence. Capture from a rich
/// Body Plan or Core-verified preparation material using the ordinary Body rules.
/// Execution material remains a separate admission. A digest received over a
/// Line cannot recreate this evidence.
/// It establishes no current membership, physical possession, or execution.
///
/// ```compile_fail
/// let _: conduit_body::BodyPlanSeal = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyPlanSeal {
    body_id: BodyId,
    wake_id: WakeId,
    plan_id: PlanId,
    workload_revision: u64,
    workset: BodyWorkset,
    plots: Vec<BodyPlotPlanSeal>,
    masks: Vec<BodyMaskTopologySeal>,
    body_time: Option<BodyTimeRequirement>,
}
impl BodyPlanSeal {
    pub fn capture(plan: &BodyPlan) -> Result<Self, BodyPlanSealRefusal> {
        use BodyPlanSealRefusal as E;
        plan.verify_seal().map_err(E::Invalid)?;
        preflight_core_plans(
            plan.plots.iter().map(|p| &p.plan).chain(
                plan.mask_topologies
                    .iter()
                    .flat_map(|m| &m.chains)
                    .map(|c| &c.plan),
            ),
        )?;
        let plots = plan.plots.iter().map(|p| capture_core(&p.plan)).collect();
        let masks = plan
            .mask_topologies
            .iter()
            .map(|m| BodyMaskTopologySeal {
                face: m.face.clone(),
                chains: m
                    .chains
                    .iter()
                    .map(|c| BodyMaskChainSeal {
                        plan: capture_core(&c.plan),
                        stages: c.stage_placement_ids.clone(),
                    })
                    .collect(),
            })
            .collect();
        Ok(Self {
            body_id: plan.body_id.clone(),
            wake_id: plan.wake_id.clone(),
            plan_id: plan.plan_id.clone(),
            workload_revision: plan.workload_revision,
            workset: plan.workset.clone(),
            plots,
            masks,
            body_time: plan.body_time_requirement.clone(),
        })
    }
    pub fn plots(&self) -> &[BodyPlotPlanSeal] {
        &self.plots
    }
    pub fn masks(&self) -> &[BodyMaskTopologySeal] {
        &self.masks
    }
    pub fn workset(&self) -> &BodyWorkset {
        &self.workset
    }
    pub fn body_time_requirement(&self) -> Option<&BodyTimeRequirement> {
        self.body_time.as_ref()
    }
}
impl private::Sealed for BodyPlanSeal {}
impl BodyPlanEvidence for BodyPlanSeal {
    fn body_id(&self) -> &BodyId {
        &self.body_id
    }
    fn wake_id(&self) -> &WakeId {
        &self.wake_id
    }
    fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    fn workset(&self) -> &BodyWorkset {
        &self.workset
    }
    fn validate_for(&self, wake: &Wake) -> Result<(), BodyPlanError> {
        if self.body_id != wake.body_id {
            return Err(BodyPlanError::WrongBody);
        }
        if self.wake_id != wake.wake_id {
            return Err(BodyPlanError::WrongWake);
        }
        if self.workload_revision != wake.workload_revision || self.workset != wake.workset {
            return Err(BodyPlanError::StaleWorkload);
        }
        // The constructor verified immutable identity, complete workset, Mask
        // paths and nested Core seals. None of those fields can be replaced.
        Ok(())
    }
}
fn capture_core(plan: &Plan) -> BodyPlotPlanSeal {
    BodyPlotPlanSeal {
        identity: PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan_id: plan.plan_id.clone(),
        fragments: plan
            .fragments
            .iter()
            .map(|f| BodyFragmentSeal {
                fragment_id: f.fragment_id.clone(),
                host_id: f.host_id.clone(),
                boot_id: f.boot_id.clone(),
                offer_generation: f.offer_generation,
            })
            .collect(),
    }
}

fn preflight_core_plans<'a>(
    cores: impl Iterator<Item = &'a Plan>,
) -> Result<(), BodyPlanSealRefusal> {
    use BodyPlanSealRefusal as E;
    let mut plans = 0usize;
    let mut fragments = 0usize;
    for core in cores {
        if !core.activations.is_empty() || !core.activation_preparations.is_empty() {
            return Err(E::NestedExecution);
        }
        plans = plans.checked_add(1).ok_or(E::Capacity)?;
        fragments = fragments
            .checked_add(core.fragments.len())
            .ok_or(E::Capacity)?;
        if plans > MAX_BODY_SEAL_PLANS || fragments > MAX_BODY_SEAL_FRAGMENTS {
            return Err(E::Capacity);
        }
    }
    Ok(())
}
