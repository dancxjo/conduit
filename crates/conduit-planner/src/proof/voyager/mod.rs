//! Finite capstone proof over the accepted Voyager planning evidence contracts.

use crate::prelude::*;
use crate::{
    DiversityRelationship, DiversityReplacementEvidence, DormantReadmissionEvidence,
    RecursiveRecoveryEvidence, ScarceResourceDisposition, ScarceResourceTriage,
    ServiceProfileAdmission, ServiceProfileDisposition, SurvivalPlanSelection,
    SurvivalPlanningMode, SurvivalPolicyRefusal,
};
use conduit_core::{verify_plan, Plan, PlanId, SignId};

mod validation;
use validation::{validate_inventory, validate_metrics, validate_phenomenon};

pub const MAXIMUM_VOYAGER_DAMAGE_STAGES: usize = 16;
pub const MAXIMUM_VOYAGER_STAGE_SIGNS: usize = 64;
pub const MAXIMUM_VOYAGER_ID_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoyagerProofClass {
    DeterministicCiFixture,
    PhysicalHil,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoyagerBodyInventory {
    pub general_purpose_hosts: u16,
    pub accelerator_hosts: u16,
    pub constrained_hosts: u16,
    pub sensor_input_capabilities: u16,
    pub presentation_capabilities: u16,
    pub line_mechanisms: u16,
    pub dormant_equipment: u16,
    pub recursive_realization_families: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoyagerStageMetrics {
    pub surviving_hosts: u16,
    pub surviving_bases: u16,
    pub surviving_lines: u16,
    pub full_capabilities: u16,
    pub degraded_capabilities: u16,
    pub unavailable_capabilities: u16,
    pub realization_gears: u16,
    pub realization_depth: u16,
    pub line_hops: u16,
    pub admitted_line_bytes: u64,
    pub estimated_item_latency_us: u64,
    pub planning_work: u32,
    pub incrementally_reused_candidates: u16,
    pub reserved_resource_units: u64,
    pub admitted_sessions: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoyagerIrrecoverableReason {
    NoSurvivingCandidate,
    HardSemanticRequirementUnsatisfied,
    AuthorityUnavailable,
    FiniteAdmissionRefused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoyagerPhenomenon {
    HealthyPreferred {
        retained_dominated_families: u16,
        activated_dominated_families: u16,
    },
    ExactRedundantReplacement {
        previous_plan_id: PlanId,
    },
    DiverseReplacement(DiversityReplacementEvidence),
    ExplicitDegradation {
        plan_id: PlanId,
        admission: ServiceProfileAdmission,
    },
    DormantReadmission(DormantReadmissionEvidence),
    RecursiveRecovery {
        lost_plan_id: PlanId,
        evidence: RecursiveRecoveryEvidence,
    },
    SurvivalPolicyDecision {
        truth_generation: u64,
        normal_refusal: SurvivalPolicyRefusal,
        survival_selection: SurvivalPlanSelection,
        scarce_resource_triage: ScarceResourceTriage,
        hard_failure_refused_under_both: bool,
    },
    Irrecoverable {
        requirement_id: String,
        reason: VoyagerIrrecoverableReason,
    },
}

#[derive(Debug, Clone)]
pub struct VoyagerDamageStage<'a> {
    pub stage_id: &'a str,
    pub observation_generation: u64,
    pub observation_signs: &'a [SignId],
    pub failed_facts: &'a [&'a str],
    pub plan: Option<&'a Plan>,
    pub previous_plan_id: Option<PlanId>,
    pub required_authority_admitted: bool,
    pub finite_resource_and_session_admission: bool,
    pub metrics: VoyagerStageMetrics,
    pub phenomena: Vec<VoyagerPhenomenon>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoyagerCapstoneEvidence {
    pub proof_class: VoyagerProofClass,
    pub stages: Vec<VoyagerStageEvidence>,
    pub historical_plan_ids: Vec<PlanId>,
    pub observation_sign_count: u16,
    pub final_metrics: VoyagerStageMetrics,
    pub exact_redundancy_observed: bool,
    pub mechanism_diversity_observed: bool,
    pub line_path_diversity_observed: bool,
    pub explicit_degradation_observed: bool,
    pub dormant_readmission_observed: bool,
    pub recursive_recovery_observed: bool,
    pub irrecoverability_observed: bool,
    pub normal_survival_divergence_observed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoyagerStageEvidence {
    pub stage_id: String,
    pub observation_generation: u64,
    pub observation_signs: Vec<SignId>,
    pub failed_facts: Vec<String>,
    pub plan_id: Option<PlanId>,
    pub host_ids: Vec<String>,
    pub implementation_ids: Vec<String>,
    pub line_ids: Vec<String>,
    pub resource_binding_count: usize,
    pub authority_binding_count: usize,
    pub metrics: VoyagerStageMetrics,
    pub scars: Vec<VoyagerScarKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoyagerScarKind {
    HealthyPreferred,
    ExactRedundancy,
    MechanismReroute,
    LinePathReroute,
    ExplicitDegradation { profile_id: String },
    DormantReadmission { host_id: String },
    RecursiveRecovery { semantic_profile: String },
    SurvivalPolicy { policy_id: String },
    Irrecoverable { requirement_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoyagerCapstoneRefusal {
    InvalidInventory,
    InvalidStageSequence,
    InvalidPlanHistory,
    MissingFreshObservation,
    AuthorityOrAdmissionBypassed,
    IncoherentTypedEvidence,
    RequiredPhenomenonMissing,
    EvidenceOverflow,
}

pub fn prove_voyager_capstone(
    proof_class: VoyagerProofClass,
    inventory: &VoyagerBodyInventory,
    stages: &[VoyagerDamageStage<'_>],
) -> Result<VoyagerCapstoneEvidence, VoyagerCapstoneRefusal> {
    validate_inventory(inventory)?;
    if stages.len() < 7 || stages.len() > MAXIMUM_VOYAGER_DAMAGE_STAGES {
        return Err(VoyagerCapstoneRefusal::InvalidStageSequence);
    }
    let mut stage_ids = alloc::collections::BTreeSet::new();
    let mut signs = alloc::collections::BTreeSet::new();
    let mut plans: Vec<Plan> = Vec::new();
    let mut previous_generation = 0_u64;
    let mut last_plan_id = None;
    let mut observed = ObservedPhenomena::default();

    for (index, stage) in stages.iter().enumerate() {
        if !valid_id(stage.stage_id)
            || !stage_ids.insert(stage.stage_id)
            || stage.observation_generation <= previous_generation
            || stage.observation_signs.is_empty()
            || stage.observation_signs.len() > MAXIMUM_VOYAGER_STAGE_SIGNS
            || stage.failed_facts.iter().any(|fact| !valid_id(fact))
            || stage.phenomena.is_empty()
        {
            return Err(VoyagerCapstoneRefusal::InvalidStageSequence);
        }
        previous_generation = stage.observation_generation;
        if stage
            .observation_signs
            .iter()
            .any(|sign| !signs.insert(sign.clone()))
        {
            return Err(VoyagerCapstoneRefusal::MissingFreshObservation);
        }
        if !stage.required_authority_admitted || !stage.finite_resource_and_session_admission {
            return Err(VoyagerCapstoneRefusal::AuthorityOrAdmissionBypassed);
        }
        validate_metrics(&stage.metrics)?;
        if let Some(plan) = stage.plan {
            if !verify_plan(plan) {
                return Err(VoyagerCapstoneRefusal::InvalidPlanHistory);
            }
            if let Some(first) = plans.first() {
                if plan.source_document_id != first.source_document_id
                    || plan.checked_form_id != first.checked_form_id
                {
                    return Err(VoyagerCapstoneRefusal::InvalidPlanHistory);
                }
            }
            if stage.previous_plan_id != last_plan_id {
                return Err(VoyagerCapstoneRefusal::InvalidPlanHistory);
            }
            if last_plan_id.as_ref() != Some(&plan.plan_id) {
                if plans
                    .iter()
                    .any(|known: &Plan| known.plan_id == plan.plan_id)
                {
                    return Err(VoyagerCapstoneRefusal::InvalidPlanHistory);
                }
                plans.push(plan.clone());
            }
            last_plan_id = Some(plan.plan_id.clone());
        } else if stage.previous_plan_id != last_plan_id {
            return Err(VoyagerCapstoneRefusal::InvalidPlanHistory);
        }
        for phenomenon in &stage.phenomena {
            validate_phenomenon(index, stage, phenomenon, &plans, &mut observed)?;
        }
    }
    if !observed.complete() {
        return Err(VoyagerCapstoneRefusal::RequiredPhenomenonMissing);
    }
    Ok(VoyagerCapstoneEvidence {
        proof_class,
        stages: stages.iter().map(stage_evidence).collect(),
        historical_plan_ids: plans.into_iter().map(|plan| plan.plan_id).collect(),
        observation_sign_count: signs
            .len()
            .try_into()
            .map_err(|_| VoyagerCapstoneRefusal::EvidenceOverflow)?,
        final_metrics: stages
            .last()
            .expect("stage count was validated")
            .metrics
            .clone(),
        exact_redundancy_observed: observed.redundancy,
        mechanism_diversity_observed: observed.mechanism,
        line_path_diversity_observed: observed.path,
        explicit_degradation_observed: observed.degradation,
        dormant_readmission_observed: observed.dormant,
        recursive_recovery_observed: observed.recursive,
        irrecoverability_observed: observed.irrecoverable,
        normal_survival_divergence_observed: observed.policy,
    })
}

fn stage_evidence(stage: &VoyagerDamageStage<'_>) -> VoyagerStageEvidence {
    let mut scars = Vec::new();
    for phenomenon in &stage.phenomena {
        match phenomenon {
            VoyagerPhenomenon::HealthyPreferred { .. } => {
                scars.push(VoyagerScarKind::HealthyPreferred)
            }
            VoyagerPhenomenon::ExactRedundantReplacement { .. } => {
                scars.push(VoyagerScarKind::ExactRedundancy)
            }
            VoyagerPhenomenon::DiverseReplacement(evidence) => match evidence.relationship {
                DiversityRelationship::MechanismDiverse => {
                    scars.push(VoyagerScarKind::MechanismReroute)
                }
                DiversityRelationship::LinePathDiverse => {
                    scars.push(VoyagerScarKind::LinePathReroute)
                }
                DiversityRelationship::MechanismAndLinePathDiverse => {
                    scars.push(VoyagerScarKind::MechanismReroute);
                    scars.push(VoyagerScarKind::LinePathReroute);
                }
                _ => {}
            },
            VoyagerPhenomenon::ExplicitDegradation { admission, .. } => {
                scars.push(VoyagerScarKind::ExplicitDegradation {
                    profile_id: admission.profile_id.clone(),
                });
            }
            VoyagerPhenomenon::DormantReadmission(evidence) => {
                scars.push(VoyagerScarKind::DormantReadmission {
                    host_id: evidence.candidate.host_id.as_str().into(),
                });
            }
            VoyagerPhenomenon::RecursiveRecovery { evidence, .. } => {
                scars.push(VoyagerScarKind::RecursiveRecovery {
                    semantic_profile: evidence.semantic_profile.clone(),
                });
            }
            VoyagerPhenomenon::SurvivalPolicyDecision {
                survival_selection, ..
            } => scars.push(VoyagerScarKind::SurvivalPolicy {
                policy_id: survival_selection.policy_id.clone(),
            }),
            VoyagerPhenomenon::Irrecoverable { requirement_id, .. } => {
                scars.push(VoyagerScarKind::Irrecoverable {
                    requirement_id: requirement_id.clone(),
                });
            }
        }
    }
    let mut host_ids = Vec::new();
    let mut implementation_ids = Vec::new();
    let mut line_ids = Vec::new();
    let mut resource_binding_count = 0;
    let mut authority_binding_count = 0;
    if let Some(plan) = stage.plan {
        for fragment in &plan.fragments {
            host_ids.push(fragment.host_id.as_str().to_string());
            for placement in &fragment.placements {
                implementation_ids.push(placement.implementation_id.as_str().to_string());
                resource_binding_count += placement.resources.len();
                authority_binding_count += placement.authority.len();
            }
            for line in fragment
                .connections
                .iter()
                .flat_map(|connection| &connection.admitted_lines)
            {
                line_ids.push(line.line_id.as_str().to_string());
            }
        }
    }
    host_ids.sort();
    host_ids.dedup();
    implementation_ids.sort();
    implementation_ids.dedup();
    line_ids.sort();
    line_ids.dedup();
    VoyagerStageEvidence {
        stage_id: stage.stage_id.into(),
        observation_generation: stage.observation_generation,
        observation_signs: stage.observation_signs.to_vec(),
        failed_facts: stage
            .failed_facts
            .iter()
            .map(|fact| (*fact).into())
            .collect(),
        plan_id: stage.plan.map(|plan| plan.plan_id.clone()),
        host_ids,
        implementation_ids,
        line_ids,
        resource_binding_count,
        authority_binding_count,
        metrics: stage.metrics.clone(),
        scars,
    }
}

#[derive(Default)]
pub(super) struct ObservedPhenomena {
    pub(super) healthy: bool,
    pub(super) redundancy: bool,
    pub(super) mechanism: bool,
    pub(super) path: bool,
    pub(super) degradation: bool,
    pub(super) dormant: bool,
    pub(super) recursive: bool,
    pub(super) irrecoverable: bool,
    pub(super) policy: bool,
}

impl ObservedPhenomena {
    fn complete(&self) -> bool {
        self.healthy
            && self.redundancy
            && self.mechanism
            && self.path
            && self.degradation
            && self.dormant
            && self.recursive
            && self.irrecoverable
            && self.policy
    }
}

pub(super) fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_VOYAGER_ID_BYTES
}
