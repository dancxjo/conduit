use crate::fact_policy::validate_predicates;
use crate::prelude::*;
use crate::{
    select_realization_with_characteristics_and_signs, HardRealizationRequirements, PlannerError,
    RealizationDecisionDisposition, RealizationPolicy, RealizationPreference, RealizationRejection,
    RealizationSelection, MAXIMUM_PLANNER_POLICY_CLAUSES,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{HostAdvertisement, RealizationAdvertisement, ResourceObservation, SignId};
use conduit_form::CheckedGear;

pub const MAXIMUM_POLICY_SOURCES: usize = 16;
pub const MAXIMUM_RETAINED_POLICY_OBSERVATIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolicySourceId(String);

impl PolicySourceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PolicySourceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyScope {
    SemanticRequirements,
    BodyWake,
    SiteDeployment,
    UserWorkspace,
    NamedStyle,
    OneShotOverride,
}

impl PolicyScope {
    fn soft_precedence(self) -> u8 {
        match self {
            Self::OneShotOverride => 0,
            Self::NamedStyle => 1,
            Self::UserWorkspace => 2,
            Self::SiteDeployment => 3,
            Self::BodyWake => 4,
            Self::SemanticRequirements => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolicySourceRevision {
    pub source_id: PolicySourceId,
    pub revision: u64,
    pub scope: PolicyScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyLayer {
    pub source: PolicySourceRevision,
    pub hard_predicates: Vec<crate::PlannerPredicate>,
    pub preferences: Vec<RealizationPreference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedObservation {
    pub observation: ResourceObservation,
    pub source: PolicySourceRevision,
    pub observed_epoch: u64,
    pub valid_through_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationBasis {
    pub sign_id: SignId,
    pub source: PolicySourceRevision,
    pub observed_epoch: u64,
    pub valid_through_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningPolicyBasis {
    pub policy_sources: Vec<PolicySourceRevision>,
    pub observations: Vec<ObservationBasis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedRealizationSelection {
    pub selection: RealizationSelection,
    pub basis: PlanningPolicyBasis,
}

struct ComposedPolicy {
    requirements: HardRealizationRequirements,
    policy: RealizationPolicy,
    hard_sources: Vec<PolicySourceRevision>,
    preference_sources: Vec<PolicySourceRevision>,
    applied_sources: Vec<PolicySourceRevision>,
}

/// Composes explicit finite policy layers and selects through the ordinary R2
/// evaluator. Layers never mutate an active Plan or become runtime state.
#[allow(clippy::too_many_arguments)]
pub fn select_realization_with_scoped_policy(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    base_requirements: &HardRealizationRequirements,
    base_requirement_source: PolicySourceRevision,
    layers: &[PolicyLayer],
    retained_observations: &[ReviewedObservation],
    current_observation_epoch: u64,
) -> Result<ScopedRealizationSelection, PlannerError> {
    let composed = compose(base_requirements, base_requirement_source.clone(), layers)?;
    let (observations, observation_basis) =
        fresh_observations(hosts, retained_observations, current_observation_epoch)?;
    let mut selection = select_realization_with_characteristics_and_signs(
        gear,
        hosts,
        advertisements,
        &composed.requirements,
        &observations,
        &composed.policy,
    )?;
    attach_sources(
        &mut selection,
        &composed.hard_sources,
        &composed.preference_sources,
        &base_requirement_source,
    );
    Ok(ScopedRealizationSelection {
        selection,
        basis: PlanningPolicyBasis {
            policy_sources: composed.applied_sources,
            observations: observation_basis,
        },
    })
}

fn compose(
    base: &HardRealizationRequirements,
    base_source: PolicySourceRevision,
    layers: &[PolicyLayer],
) -> Result<ComposedPolicy, PlannerError> {
    validate_source(&base_source)?;
    if layers.len() + 1 > MAXIMUM_POLICY_SOURCES {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "policy has {} sources above the bound of {}",
            layers.len() + 1,
            MAXIMUM_POLICY_SOURCES
        )));
    }
    let mut source_revisions = BTreeMap::new();
    source_revisions.insert(base_source.source_id.clone(), base_source.revision);
    for layer in layers {
        validate_source(&layer.source)?;
        if source_revisions
            .insert(layer.source.source_id.clone(), layer.source.revision)
            .is_some()
        {
            return invalid_policy("a policy source may appear only once in one planning basis");
        }
    }

    let mut requirements = base.clone();
    let mut hard_sources = vec![base_source.clone(); requirements.predicates.len()];
    let mut ordered_layers = layers.iter().collect::<Vec<_>>();
    ordered_layers.sort_by(|left, right| {
        left.source
            .scope
            .soft_precedence()
            .cmp(&right.source.scope.soft_precedence())
            .then_with(|| left.source.source_id.cmp(&right.source.source_id))
            .then_with(|| left.source.revision.cmp(&right.source.revision))
    });
    for layer in &ordered_layers {
        requirements
            .predicates
            .extend(layer.hard_predicates.iter().cloned());
        hard_sources.extend(vec![layer.source.clone(); layer.hard_predicates.len()]);
    }
    if requirements.predicates.len() > MAXIMUM_PLANNER_POLICY_CLAUSES {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "composed hard policy exceeds the {} clause bound",
            MAXIMUM_PLANNER_POLICY_CLAUSES
        )));
    }
    validate_predicates(&requirements.predicates).map_err(|error| {
        PlannerError::InvalidHardRealizationRequirement(format!(
            "composed hard policy conflicts: {error:?}"
        ))
    })?;

    let mut preferences = Vec::new();
    let mut preference_sources = Vec::new();
    for layer in &ordered_layers {
        preferences.extend(layer.preferences.iter().cloned());
        preference_sources.extend(vec![layer.source.clone(); layer.preferences.len()]);
    }
    if preferences.len() > MAXIMUM_PLANNER_POLICY_CLAUSES {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "composed soft policy exceeds the {} clause bound",
            MAXIMUM_PLANNER_POLICY_CLAUSES
        )));
    }
    let mut applied_sources = vec![base_source];
    applied_sources.extend(ordered_layers.into_iter().map(|layer| layer.source.clone()));
    Ok(ComposedPolicy {
        requirements,
        policy: RealizationPolicy { preferences },
        hard_sources,
        preference_sources,
        applied_sources,
    })
}

fn fresh_observations(
    hosts: &[HostAdvertisement],
    retained: &[ReviewedObservation],
    current_epoch: u64,
) -> Result<(Vec<ResourceObservation>, Vec<ObservationBasis>), PlannerError> {
    if retained.len() > MAXIMUM_RETAINED_POLICY_OBSERVATIONS {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "retained policy observations exceed the {} item bound",
            MAXIMUM_RETAINED_POLICY_OBSERVATIONS
        )));
    }
    let mut signs = BTreeSet::new();
    let mut observations = Vec::new();
    let mut basis = Vec::new();
    for reviewed in retained {
        validate_source(&reviewed.source)?;
        if reviewed.observation.sign_id.as_str().is_empty()
            || !signs.insert(reviewed.observation.sign_id.clone())
        {
            return Err(PlannerError::InvalidResourceObservation(
                "reviewed observation Sign identities must be non-empty and unique".into(),
            ));
        }
        if reviewed.observed_epoch > reviewed.valid_through_epoch {
            return Err(PlannerError::InvalidResourceObservation(
                "reviewed observation freshness interval is inverted".into(),
            ));
        }
        if reviewed.observed_epoch <= current_epoch && current_epoch <= reviewed.valid_through_epoch
        {
            observations.push(reviewed.observation.clone());
            basis.push(ObservationBasis {
                sign_id: reviewed.observation.sign_id.clone(),
                source: reviewed.source.clone(),
                observed_epoch: reviewed.observed_epoch,
                valid_through_epoch: reviewed.valid_through_epoch,
            });
        }
    }
    crate::observations::validate_resource_observations(hosts, &observations)?;
    Ok((observations, basis))
}

fn attach_sources(
    selection: &mut RealizationSelection,
    hard_sources: &[PolicySourceRevision],
    preference_sources: &[PolicySourceRevision],
    base_source: &PolicySourceRevision,
) {
    for record in &mut selection.signs {
        if let RealizationDecisionDisposition::Rejected(rejection) = &record.disposition {
            record.clause_source = match rejection {
                RealizationRejection::HardPredicate { clause_index, .. } => {
                    hard_sources.get(usize::from(*clause_index)).cloned()
                }
                _ => Some(base_source.clone()),
            };
        }
        record.decisive_preference_source = record
            .decisive_preference_clause
            .and_then(|index| preference_sources.get(usize::from(index)).cloned());
    }
}

fn validate_source(source: &PolicySourceRevision) -> Result<(), PlannerError> {
    if source.source_id.as_str().is_empty() || source.revision == 0 {
        return invalid_policy("policy source identity must be non-empty and revision non-zero");
    }
    Ok(())
}

fn invalid_policy<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(detail.into()))
}
