use super::{
    ensure_compatible, FactPolicyError, PlannerFactRef, PlannerFactValue, PlannerPredicate,
    PlannerPreference,
};

impl PlannerFactRef {
    fn identity_is_empty(&self) -> bool {
        match self {
            Self::RealizationCharacteristic(id) => id.as_str().is_empty(),
            Self::ResourceUnits(id)
            | Self::ComputeServiceGuarantee(id)
            | Self::ObservationUnreservedUnits(id)
            | Self::ObservationUtilizedUnits(id) => id.as_str().is_empty(),
            Self::ComputePerformanceClass {
                resource_class_id,
                topology_group_id,
            }
            | Self::ComputeNominalClockHz {
                resource_class_id,
                topology_group_id,
            } => resource_class_id.as_str().is_empty() || topology_group_id.as_str().is_empty(),
            Self::ComputeHasPerformanceClass {
                resource_class_id,
                performance_class_id,
            } => resource_class_id.as_str().is_empty() || performance_class_id.as_str().is_empty(),
            Self::RequiresAuthority(id) => id.as_str().is_empty(),
            Self::RequiresHostOperation(id) => id.as_str().is_empty(),
            Self::HostIdentity | Self::OfferQueueItems | Self::OfferQueueBytes => false,
        }
    }
}

pub(crate) fn validate_predicates(predicates: &[PlannerPredicate]) -> Result<(), FactPolicyError> {
    for predicate in predicates {
        if predicate.fact().identity_is_empty() {
            return Err(FactPolicyError::EmptyIdentity);
        }
        if let PlannerPredicate::In { values, .. } = predicate {
            validate_values(values)?;
        }
    }
    for (index, left) in predicates.iter().enumerate() {
        for right in &predicates[index + 1..] {
            if right == left {
                return Err(FactPolicyError::DuplicateValue);
            }
            if contradictory(left, right)? {
                return Err(FactPolicyError::ContradictoryPredicate);
            }
        }
    }
    Ok(())
}

fn contradictory(
    left: &PlannerPredicate,
    right: &PlannerPredicate,
) -> Result<bool, FactPolicyError> {
    if left.fact() != right.fact() {
        return Ok(false);
    }
    let contradiction = match (left, right) {
        (PlannerPredicate::Absent { .. }, _) | (_, PlannerPredicate::Absent { .. }) => true,
        (
            PlannerPredicate::Equal { value: left, .. },
            PlannerPredicate::Equal { value: right, .. },
        ) => {
            ensure_compatible(left, right, false)?;
            left != right
        }
        (
            PlannerPredicate::Equal { value: left, .. },
            PlannerPredicate::NotEqual { value: right, .. },
        )
        | (
            PlannerPredicate::NotEqual { value: right, .. },
            PlannerPredicate::Equal { value: left, .. },
        ) => {
            ensure_compatible(left, right, false)?;
            left == right
        }
        (
            PlannerPredicate::AtLeast { value: minimum, .. },
            PlannerPredicate::AtMost { value: maximum, .. },
        )
        | (
            PlannerPredicate::AtMost { value: maximum, .. },
            PlannerPredicate::AtLeast { value: minimum, .. },
        ) => {
            ensure_compatible(minimum, maximum, true)?;
            minimum > maximum
        }
        (PlannerPredicate::In { values: left, .. }, PlannerPredicate::In { values: right, .. }) => {
            for left_value in left {
                for right_value in right {
                    ensure_compatible(left_value, right_value, false)?;
                }
            }
            !left.iter().any(|value| right.contains(value))
        }
        _ => false,
    };
    Ok(contradiction)
}

pub(crate) fn validate_preference(preference: &PlannerPreference) -> Result<(), FactPolicyError> {
    if preference.fact().identity_is_empty() {
        return Err(FactPolicyError::EmptyIdentity);
    }
    if let PlannerPreference::PreferOrder { values, .. } = preference {
        validate_values(values)?;
    }
    Ok(())
}

pub(super) fn validate_values(values: &[PlannerFactValue]) -> Result<(), FactPolicyError> {
    if values.is_empty() {
        return Err(FactPolicyError::EmptyValues);
    }
    if values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
    {
        return Err(FactPolicyError::DuplicateValue);
    }
    Ok(())
}
