use crate::prelude::*;
use crate::{FactDomain, PlanningFactKey};
use alloc::collections::BTreeSet;
use conduit_core::{
    ConnectionBaseInstanceId, ConnectionId, GearId, ImplementationId, LineId, Plan,
};

pub const MAXIMUM_DIVERSITY_CANDIDATES: usize = 16;
pub const MAXIMUM_DIVERSITY_DEPENDENCIES: usize = 32;
pub const MAXIMUM_DIVERSITY_MECHANISMS: usize = 16;
pub const MAXIMUM_DIVERSITY_LINE_HOPS: usize = 16;
pub const MAXIMUM_DIVERSITY_ID_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MechanismDependency {
    pub gear_id: GearId,
    pub implementation_id: ImplementationId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LinePathHop {
    pub connection_id: ConnectionId,
    pub line_id: LineId,
    pub base_instance_id: ConnectionBaseInstanceId,
}

/// Reviewed diversity knowledge. Dependencies name exact current facts; the
/// mechanism and path fields must also agree with the ordinary sealed Plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiversityCandidate {
    pub candidate_id: String,
    pub semantic_capability_id: String,
    pub semantic_cord_id: String,
    pub policy_rank: u64,
    pub critical_dependencies: Vec<PlanningFactKey>,
    pub mechanisms: Vec<MechanismDependency>,
    pub line_path: Vec<LinePathHop>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiversityRelationship {
    SameRealization,
    DifferentButSharedCriticalDependency,
    MechanismDiverse,
    LinePathDiverse,
    MechanismAndLinePathDiverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviousPlanDisposition {
    InvalidatedRequiresTermination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiversityReplacementEvidence {
    pub semantic_capability_id: String,
    pub semantic_cord_id: String,
    pub previous_candidate_id: String,
    pub replacement_candidate_id: String,
    pub previous_plan_id: conduit_core::PlanId,
    pub replacement_plan_id: conduit_core::PlanId,
    pub previous_plan_disposition: PreviousPlanDisposition,
    pub unavailable_previous_dependencies: Vec<PlanningFactKey>,
    pub relationship: DiversityRelationship,
    pub replacement_mechanisms: Vec<MechanismDependency>,
    pub replacement_line_path: Vec<LinePathHop>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiversityRefusal {
    InvalidCandidate,
    SemanticIdentityChanged,
    PreviousStillAvailable,
    ReplacementUnavailable,
    SharedCriticalDependency,
    CosmeticAlternative,
    PlanIdentityUnchanged,
    FormIdentityChanged,
    PlanDoesNotSealCandidate,
    NoSurvivingCandidate,
}

pub fn classify_diversity(
    left: &DiversityCandidate,
    right: &DiversityCandidate,
) -> Result<DiversityRelationship, DiversityRefusal> {
    validate_candidate(left)?;
    validate_candidate(right)?;
    same_semantics(left, right)?;
    if left.mechanisms == right.mechanisms && left.line_path == right.line_path {
        return Ok(DiversityRelationship::SameRealization);
    }
    let left_dependencies = left.critical_dependencies.iter().collect::<BTreeSet<_>>();
    if right
        .critical_dependencies
        .iter()
        .any(|dependency| left_dependencies.contains(dependency))
    {
        return Ok(DiversityRelationship::DifferentButSharedCriticalDependency);
    }
    match (
        left.mechanisms != right.mechanisms,
        left.line_path != right.line_path,
    ) {
        (true, true) => Ok(DiversityRelationship::MechanismAndLinePathDiverse),
        (true, false) => Ok(DiversityRelationship::MechanismDiverse),
        (false, true) => Ok(DiversityRelationship::LinePathDiverse),
        (false, false) => Ok(DiversityRelationship::SameRealization),
    }
}

/// Chooses only among reviewed candidates whose every critical dependency is
/// current. Rank is ordinary policy input; it is not a reliability score.
pub fn select_surviving_diverse_candidate<'a>(
    previous: &DiversityCandidate,
    candidates: &'a [DiversityCandidate],
    current_dependencies: &[PlanningFactKey],
) -> Result<&'a DiversityCandidate, DiversityRefusal> {
    validate_candidate(previous)?;
    if candidates.is_empty() || candidates.len() > MAXIMUM_DIVERSITY_CANDIDATES {
        return Err(DiversityRefusal::InvalidCandidate);
    }
    let current = current_dependencies.iter().collect::<BTreeSet<_>>();
    if available(previous, &current) {
        return Err(DiversityRefusal::PreviousStillAvailable);
    }
    let mut surviving = Vec::new();
    let mut shared_survivor = false;
    let mut cosmetic_survivor = false;
    for candidate in candidates {
        validate_candidate(candidate)?;
        same_semantics(previous, candidate)?;
        if !available(candidate, &current) {
            continue;
        }
        match classify_diversity(previous, candidate)? {
            DiversityRelationship::MechanismDiverse
            | DiversityRelationship::LinePathDiverse
            | DiversityRelationship::MechanismAndLinePathDiverse => surviving.push(candidate),
            DiversityRelationship::DifferentButSharedCriticalDependency => shared_survivor = true,
            DiversityRelationship::SameRealization => cosmetic_survivor = true,
        }
    }
    surviving.sort_by_key(|candidate| (candidate.policy_rank, candidate.candidate_id.as_str()));
    if let Some(candidate) = surviving.into_iter().next() {
        Ok(candidate)
    } else if shared_survivor {
        Err(DiversityRefusal::SharedCriticalDependency)
    } else if cosmetic_survivor {
        Err(DiversityRefusal::CosmeticAlternative)
    } else {
        Err(DiversityRefusal::NoSurvivingCandidate)
    }
}

pub fn prove_diverse_replacement(
    previous_plan: &Plan,
    replacement_plan: &Plan,
    previous: &DiversityCandidate,
    replacement: &DiversityCandidate,
    current_dependencies: &[PlanningFactKey],
) -> Result<DiversityReplacementEvidence, DiversityRefusal> {
    validate_candidate(previous)?;
    validate_candidate(replacement)?;
    same_semantics(previous, replacement)?;
    if previous_plan.plan_id == replacement_plan.plan_id {
        return Err(DiversityRefusal::PlanIdentityUnchanged);
    }
    if previous_plan.source_document_id != replacement_plan.source_document_id
        || previous_plan.checked_form_id != replacement_plan.checked_form_id
        || previous_plan.expanded_form_id != replacement_plan.expanded_form_id
    {
        return Err(DiversityRefusal::FormIdentityChanged);
    }
    if !plan_seals(previous_plan, previous) || !plan_seals(replacement_plan, replacement) {
        return Err(DiversityRefusal::PlanDoesNotSealCandidate);
    }
    let current = current_dependencies.iter().collect::<BTreeSet<_>>();
    let unavailable_previous_dependencies = previous
        .critical_dependencies
        .iter()
        .filter(|dependency| !current.contains(*dependency))
        .cloned()
        .collect::<Vec<_>>();
    if unavailable_previous_dependencies.is_empty() {
        return Err(DiversityRefusal::PreviousStillAvailable);
    }
    if !available(replacement, &current) {
        return Err(DiversityRefusal::ReplacementUnavailable);
    }
    let relationship = classify_diversity(previous, replacement)?;
    match relationship {
        DiversityRelationship::DifferentButSharedCriticalDependency => {
            return Err(DiversityRefusal::SharedCriticalDependency)
        }
        DiversityRelationship::SameRealization => {
            return Err(DiversityRefusal::CosmeticAlternative)
        }
        DiversityRelationship::MechanismDiverse
        | DiversityRelationship::LinePathDiverse
        | DiversityRelationship::MechanismAndLinePathDiverse => {}
    }
    Ok(DiversityReplacementEvidence {
        semantic_capability_id: previous.semantic_capability_id.clone(),
        semantic_cord_id: previous.semantic_cord_id.clone(),
        previous_candidate_id: previous.candidate_id.clone(),
        replacement_candidate_id: replacement.candidate_id.clone(),
        previous_plan_id: previous_plan.plan_id.clone(),
        replacement_plan_id: replacement_plan.plan_id.clone(),
        previous_plan_disposition: PreviousPlanDisposition::InvalidatedRequiresTermination,
        unavailable_previous_dependencies,
        relationship,
        replacement_mechanisms: replacement.mechanisms.clone(),
        replacement_line_path: replacement.line_path.clone(),
    })
}

fn validate_candidate(candidate: &DiversityCandidate) -> Result<(), DiversityRefusal> {
    let valid_id = |value: &str| !value.is_empty() && value.len() <= MAXIMUM_DIVERSITY_ID_BYTES;
    let dependencies = candidate
        .critical_dependencies
        .iter()
        .collect::<BTreeSet<_>>();
    let mechanisms = candidate.mechanisms.iter().collect::<BTreeSet<_>>();
    let hops = candidate.line_path.iter().collect::<BTreeSet<_>>();
    let seals_every_mechanism_dependency = candidate.mechanisms.iter().all(|mechanism| {
        dependencies.contains(&PlanningFactKey::exact(
            FactDomain::Implementation,
            mechanism.implementation_id.as_str(),
        ))
    });
    let seals_every_line_dependency = candidate.line_path.iter().all(|hop| {
        dependencies.contains(&PlanningFactKey::exact(
            FactDomain::Line,
            hop.base_instance_id.as_str(),
        ))
    });
    if !valid_id(&candidate.candidate_id)
        || !valid_id(&candidate.semantic_capability_id)
        || !valid_id(&candidate.semantic_cord_id)
        || candidate.critical_dependencies.is_empty()
        || candidate.critical_dependencies.len() > MAXIMUM_DIVERSITY_DEPENDENCIES
        || dependencies.len() != candidate.critical_dependencies.len()
        || !seals_every_mechanism_dependency
        || !seals_every_line_dependency
        || candidate.critical_dependencies.iter().any(|dependency| {
            dependency.identity.is_empty()
                || dependency.identity.len() > MAXIMUM_DIVERSITY_ID_BYTES
                || matches!(dependency.domain, FactDomain::Semantic | FactDomain::Policy)
        })
        || candidate.mechanisms.is_empty()
        || candidate.mechanisms.len() > MAXIMUM_DIVERSITY_MECHANISMS
        || mechanisms.len() != candidate.mechanisms.len()
        || candidate.line_path.is_empty()
        || candidate.line_path.len() > MAXIMUM_DIVERSITY_LINE_HOPS
        || hops.len() != candidate.line_path.len()
    {
        return Err(DiversityRefusal::InvalidCandidate);
    }
    Ok(())
}

fn same_semantics(
    left: &DiversityCandidate,
    right: &DiversityCandidate,
) -> Result<(), DiversityRefusal> {
    if left.semantic_capability_id != right.semantic_capability_id
        || left.semantic_cord_id != right.semantic_cord_id
    {
        return Err(DiversityRefusal::SemanticIdentityChanged);
    }
    Ok(())
}

fn available(candidate: &DiversityCandidate, current: &BTreeSet<&PlanningFactKey>) -> bool {
    candidate
        .critical_dependencies
        .iter()
        .all(|dependency| current.contains(dependency))
}

fn plan_seals(plan: &Plan, candidate: &DiversityCandidate) -> bool {
    candidate.mechanisms.iter().all(|mechanism| {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .any(|placement| {
                placement.gear_id == mechanism.gear_id
                    && placement.implementation_id == mechanism.implementation_id
            })
    }) && candidate.line_path.iter().all(|hop| {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .filter(|connection| connection.connection_id == hop.connection_id)
            .flat_map(|connection| &connection.admitted_lines)
            .any(|line| {
                line.line_id == hop.line_id && line.binding.base_instance_id == hop.base_instance_id
            })
    })
}
