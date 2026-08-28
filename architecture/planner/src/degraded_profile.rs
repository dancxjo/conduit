//! Explicit admission of reviewed weaker service profiles.

use crate::prelude::*;
use crate::{
    select_realization_with_characteristics_and_signs, HardRealizationRequirements, PlannerError,
    PlannerFactRef, PlannerFactValue, PlannerPredicate, RealizationDecisionRecord,
    RealizationPolicy,
};
use conduit_core::{
    BaseImplementationId, CharacteristicId, HostAdvertisement, Plan, RealizationAdvertisement,
    ResourceObservation, SignId,
};
use conduit_form::{CheckedForm, CheckedGear};

pub const MAXIMUM_DEGRADED_PROFILE_DIMENSIONS: usize = 16;
pub const MAXIMUM_DEGRADED_PROFILE_ID_BYTES: usize = 256;
pub const MAXIMUM_DEGRADED_PROFILE_LABEL_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationDirection {
    HigherIsStronger,
    LowerIsStronger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradedDimension {
    pub characteristic_id: CharacteristicId,
    pub human_name: String,
    pub full_value: PlannerFactValue,
    pub weakest_permitted_value: PlannerFactValue,
    pub direction: DegradationDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedServiceProfile {
    pub profile_id: String,
    /// Requirements that survival policy can never relax.
    pub hard_requirements: HardRealizationRequirements,
    /// Exact provenance/evidence class required for every admitted output.
    pub required_evidence: Option<(CharacteristicId, PlannerFactValue)>,
    /// Kind-reviewed soft dimensions, each with an exact relaxation boundary.
    pub degradable_dimensions: Vec<DegradedDimension>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurvivalPolicy {
    pub policy_id: String,
    pub revision: u64,
    pub permitted_profile_id: String,
    pub permitted_dimensions: Vec<CharacteristicId>,
    pub degradation_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceProfileDisposition {
    Full,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProfileAdmission {
    pub disposition: ServiceProfileDisposition,
    pub profile_id: String,
    pub policy_id: Option<String>,
    pub policy_revision: Option<u64>,
    pub choice: crate::PlacementChoice,
    pub decisions: Vec<RealizationDecisionRecord>,
    pub dimensions: Vec<DegradedDimensionEvidence>,
    pub observation_signs: Vec<SignId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradedDimensionEvidence {
    pub characteristic_id: CharacteristicId,
    pub human_name: String,
    pub requested_value: PlannerFactValue,
    pub weakest_permitted_value: PlannerFactValue,
    pub admitted_value: PlannerFactValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradedProfileRefusal {
    InvalidProfile,
    InvalidPolicy,
    DegradationForbidden,
    PolicyOutsideReviewedBounds,
    HardRequirementUnsatisfied,
    MissingRequiredEvidence,
    SemanticallyDifferentDimension,
    NoMeaningfulWeakerProfile,
    StaleOrMissingObservation,
    Unrealizable,
    Planner(PlannerError),
}

pub fn select_reviewed_service_profile(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
    profile: &ReviewedServiceProfile,
    policy: Option<&SurvivalPolicy>,
    preferences: &RealizationPolicy,
) -> Result<ServiceProfileAdmission, DegradedProfileRefusal> {
    validate_profile(profile)?;
    if observations.len() > crate::MAXIMUM_RETAINED_POLICY_OBSERVATIONS {
        return Err(DegradedProfileRefusal::InvalidProfile);
    }
    let full = requirements(profile, true);
    match select_realization_with_characteristics_and_signs(
        gear,
        hosts,
        advertisements,
        &full,
        observations,
        preferences,
    ) {
        Ok(selection) => Ok(ServiceProfileAdmission {
            disposition: ServiceProfileDisposition::Full,
            profile_id: profile.profile_id.clone(),
            policy_id: None,
            policy_revision: None,
            choice: selection.choice.clone(),
            decisions: selection.signs,
            dimensions: dimension_evidence(profile, advertisements, &selection.choice)?,
            observation_signs: selected_observation_signs(observations, &selection.choice.host_id),
        }),
        Err(full_error) => {
            if !hard_failure(&full_error) {
                return Err(classify_planner_refusal(full_error, false));
            }
            // Determine whether immutable requirements, rather than one
            // reviewed full-profile target, made the semantic work impossible.
            let hard_only = select_realization_with_characteristics_and_signs(
                gear,
                hosts,
                advertisements,
                &profile.hard_requirements,
                observations,
                preferences,
            );
            if hard_only.is_err() {
                return Err(classify_planner_refusal(full_error, true));
            }
            let evidence_required = base_requirements(profile);
            if select_realization_with_characteristics_and_signs(
                gear,
                hosts,
                advertisements,
                &evidence_required,
                observations,
                preferences,
            )
            .is_err()
            {
                return Err(DegradedProfileRefusal::MissingRequiredEvidence);
            }
            let policy = policy.ok_or(DegradedProfileRefusal::DegradationForbidden)?;
            validate_policy(profile, policy)?;
            if !policy.degradation_allowed {
                return Err(DegradedProfileRefusal::DegradationForbidden);
            }
            let weaker = requirements(profile, false);
            let selection = select_realization_with_characteristics_and_signs(
                gear,
                hosts,
                advertisements,
                &weaker,
                observations,
                preferences,
            )
            .map_err(|error| classify_planner_refusal(error, false))?;
            Ok(ServiceProfileAdmission {
                disposition: ServiceProfileDisposition::Degraded,
                profile_id: profile.profile_id.clone(),
                policy_id: Some(policy.policy_id.clone()),
                policy_revision: Some(policy.revision),
                choice: selection.choice.clone(),
                decisions: selection.signs,
                dimensions: dimension_evidence(profile, advertisements, &selection.choice)?,
                observation_signs: selected_observation_signs(
                    observations,
                    &selection.choice.host_id,
                ),
            })
        }
    }
}

fn dimension_evidence(
    profile: &ReviewedServiceProfile,
    advertisements: &[RealizationAdvertisement],
    choice: &crate::PlacementChoice,
) -> Result<Vec<DegradedDimensionEvidence>, DegradedProfileRefusal> {
    let advertisement = advertisements
        .iter()
        .find(|item| item.host_id == choice.host_id && item.capability_id == choice.capability_id)
        .ok_or(DegradedProfileRefusal::Unrealizable)?;
    profile
        .degradable_dimensions
        .iter()
        .map(|dimension| {
            let actual = advertisement
                .characteristics
                .iter()
                .find(|item| item.definition.characteristic_id == dimension.characteristic_id)
                .ok_or(DegradedProfileRefusal::Unrealizable)?;
            let admitted_value = match &actual.value {
                conduit_core::CharacteristicValue::Boolean(value) => {
                    PlannerFactValue::Boolean(*value)
                }
                conduit_core::CharacteristicValue::UnsignedQuantity { value, unit } => {
                    PlannerFactValue::Quantity {
                        value: *value,
                        unit: *unit,
                    }
                }
                conduit_core::CharacteristicValue::Categorical(value) => {
                    PlannerFactValue::Category(value.clone())
                }
            };
            Ok(DegradedDimensionEvidence {
                characteristic_id: dimension.characteristic_id.clone(),
                human_name: dimension.human_name.clone(),
                requested_value: dimension.full_value.clone(),
                weakest_permitted_value: dimension.weakest_permitted_value.clone(),
                admitted_value,
            })
        })
        .collect()
}

fn selected_observation_signs(
    observations: &[ResourceObservation],
    host_id: &conduit_core::HostId,
) -> Vec<SignId> {
    let mut signs = observations
        .iter()
        .filter(|item| &item.host_id == host_id)
        .map(|item| item.sign_id.clone())
        .collect::<Vec<_>>();
    signs.sort();
    signs.dedup();
    signs
}

/// Seals an already admitted exact profile choice through the ordinary Plan path.
/// The admission does not grant authority and cannot substitute a different Form.
pub fn seal_reviewed_service_profile_plan(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    advertisements: &[RealizationAdvertisement],
    admission: &ServiceProfileAdmission,
) -> Result<Plan, DegradedProfileRefusal> {
    if form.gears.len() != 1 {
        return Err(DegradedProfileRefusal::InvalidProfile);
    }
    let gear = &form.gears[0];
    let placements = crate::PlacementChoices {
        by_gear: alloc::collections::BTreeMap::from([(
            gear.gear_id.clone(),
            admission.choice.clone(),
        )]),
    };
    let plan =
        crate::plan(form, hosts, &placements, bases).map_err(DegradedProfileRefusal::Planner)?;
    crate::characteristic_sealing::seal_characteristics(plan, advertisements)
        .map_err(DegradedProfileRefusal::Planner)
}

fn validate_profile(profile: &ReviewedServiceProfile) -> Result<(), DegradedProfileRefusal> {
    if profile.profile_id.is_empty()
        || profile.profile_id.len() > MAXIMUM_DEGRADED_PROFILE_ID_BYTES
        || profile.degradable_dimensions.is_empty()
        || profile.degradable_dimensions.len() > MAXIMUM_DEGRADED_PROFILE_DIMENSIONS
    {
        return Err(DegradedProfileRefusal::InvalidProfile);
    }
    let mut ids = alloc::collections::BTreeSet::new();
    for dimension in &profile.degradable_dimensions {
        if dimension.characteristic_id.as_str().is_empty()
            || dimension.human_name.is_empty()
            || dimension.characteristic_id.as_str().len() > MAXIMUM_DEGRADED_PROFILE_ID_BYTES
            || dimension.human_name.len() > MAXIMUM_DEGRADED_PROFILE_LABEL_BYTES
            || !ids.insert(dimension.characteristic_id.clone())
        {
            return Err(DegradedProfileRefusal::NoMeaningfulWeakerProfile);
        }
        match weaker_order(dimension)? {
            true => {}
            false => return Err(DegradedProfileRefusal::NoMeaningfulWeakerProfile),
        }
    }
    if profile.required_evidence.as_ref().is_some_and(|(id, _)| {
        id.as_str().is_empty() || id.as_str().len() > MAXIMUM_DEGRADED_PROFILE_ID_BYTES
    }) {
        return Err(DegradedProfileRefusal::InvalidProfile);
    }
    Ok(())
}

fn validate_policy(
    profile: &ReviewedServiceProfile,
    policy: &SurvivalPolicy,
) -> Result<(), DegradedProfileRefusal> {
    if policy.policy_id.is_empty()
        || policy.policy_id.len() > MAXIMUM_DEGRADED_PROFILE_ID_BYTES
        || policy.revision == 0
        || policy.permitted_profile_id != profile.profile_id
        || policy.permitted_dimensions.len() != profile.degradable_dimensions.len()
    {
        return Err(DegradedProfileRefusal::InvalidPolicy);
    }
    let permitted = policy
        .permitted_dimensions
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>();
    if permitted.len() != policy.permitted_dimensions.len()
        || profile
            .degradable_dimensions
            .iter()
            .any(|dimension| !permitted.contains(&dimension.characteristic_id))
    {
        return Err(DegradedProfileRefusal::PolicyOutsideReviewedBounds);
    }
    Ok(())
}

fn requirements(profile: &ReviewedServiceProfile, full: bool) -> HardRealizationRequirements {
    let mut requirements = base_requirements(profile);
    requirements
        .predicates
        .extend(profile.degradable_dimensions.iter().map(|dimension| {
            let value = if full {
                &dimension.full_value
            } else {
                &dimension.weakest_permitted_value
            };
            match dimension.direction {
                DegradationDirection::HigherIsStronger => PlannerPredicate::AtLeast {
                    fact: PlannerFactRef::RealizationCharacteristic(
                        dimension.characteristic_id.clone(),
                    ),
                    value: value.clone(),
                },
                DegradationDirection::LowerIsStronger => PlannerPredicate::AtMost {
                    fact: PlannerFactRef::RealizationCharacteristic(
                        dimension.characteristic_id.clone(),
                    ),
                    value: value.clone(),
                },
            }
        }));
    requirements
}

fn base_requirements(profile: &ReviewedServiceProfile) -> HardRealizationRequirements {
    let mut requirements = profile.hard_requirements.clone();
    if let Some((id, value)) = &profile.required_evidence {
        requirements.predicates.push(PlannerPredicate::Equal {
            fact: PlannerFactRef::RealizationCharacteristic(id.clone()),
            value: value.clone(),
        });
    }
    requirements
}

fn weaker_order(dimension: &DegradedDimension) -> Result<bool, DegradedProfileRefusal> {
    use core::cmp::Ordering;
    let order = match (&dimension.full_value, &dimension.weakest_permitted_value) {
        (
            PlannerFactValue::Quantity {
                value: full,
                unit: full_unit,
            },
            PlannerFactValue::Quantity {
                value: weak,
                unit: weak_unit,
            },
        ) if full_unit == weak_unit => full.cmp(weak),
        _ => return Err(DegradedProfileRefusal::SemanticallyDifferentDimension),
    };
    Ok(matches!(
        (dimension.direction, order),
        (DegradationDirection::HigherIsStronger, Ordering::Greater)
            | (DegradationDirection::LowerIsStronger, Ordering::Less)
    ))
}

fn hard_failure(error: &PlannerError) -> bool {
    matches!(
        error,
        PlannerError::HardRealizationRequirementUnsatisfied(_)
    )
}

fn classify_planner_refusal(error: PlannerError, hard_only: bool) -> DegradedProfileRefusal {
    match error {
        PlannerError::CurrentResourceObservationUnavailable(_)
        | PlannerError::InvalidResourceObservation(_)
        | PlannerError::InvalidPlanningObservation(_) => {
            DegradedProfileRefusal::StaleOrMissingObservation
        }
        PlannerError::HardRealizationRequirementUnsatisfied(_) if hard_only => {
            DegradedProfileRefusal::HardRequirementUnsatisfied
        }
        PlannerError::HardRealizationRequirementUnsatisfied(_) => {
            DegradedProfileRefusal::Unrealizable
        }
        other => DegradedProfileRefusal::Planner(other),
    }
}
