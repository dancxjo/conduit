//! Conservative composition of finite work facts and deadline/WCET facts.
//!
//! Finite capacity is useful for ordinary Forms, but it is not a timing proof.
//! This module keeps the two analyses separate: a deadline region accepts only
//! selected realizations with an explicit worst-case basis for every dependency.
//! It does not introduce a semantic-unboundedness category.

use crate::prelude::*;

const MAXIMUM_TIMING_DEPENDENCIES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingFacts {
    pub realization_id: String,
    /// A finite semantic/resource capacity. This fact is not used as WCET.
    pub finite_capacity: u64,
    pub resource_units: u32,
    /// Known operation count, when the selected realization supplies one.
    pub maximum_operations: Option<u64>,
    /// Measured or derived worst-case time on the exact selected realization.
    pub wcet_us: Option<u32>,
    pub basis_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingDependency {
    pub dependency_id: String,
    pub facts: TimingFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineRegion {
    pub region_id: String,
    pub deadline_us: u32,
    pub maximum_resource_units: u32,
    pub dependencies: Vec<TimingDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineAdmission {
    pub region_id: String,
    pub deadline_us: u32,
    pub maximum_resource_units: u32,
    pub total_wcet_us: u32,
    pub total_resource_units: u32,
    pub basis_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WcetRefusal {
    EmptyRegion,
    TooManyDependencies,
    MissingWcet { dependency_id: String },
    MissingBasis { dependency_id: String },
    WcetOverflow,
    ResourceOverflow,
    DeadlineExceeded { required_us: u32, deadline_us: u32 },
    ResourceExceeded { required: u32, maximum: u32 },
    ReplanViolatesAdmission,
}

/// Admit one exact region from the selected realization facts.
pub fn admit_deadline_region(region: &DeadlineRegion) -> Result<DeadlineAdmission, WcetRefusal> {
    if region.region_id.is_empty() || region.deadline_us == 0 || region.dependencies.is_empty() {
        return Err(WcetRefusal::EmptyRegion);
    }
    if region.dependencies.len() > MAXIMUM_TIMING_DEPENDENCIES {
        return Err(WcetRefusal::TooManyDependencies);
    }

    let mut total_wcet_us = 0u32;
    let mut total_resource_units = 0u32;
    let mut basis_ids = Vec::with_capacity(region.dependencies.len());
    for dependency in &region.dependencies {
        let wcet_us = dependency
            .facts
            .wcet_us
            .ok_or_else(|| WcetRefusal::MissingWcet {
                dependency_id: dependency.dependency_id.clone(),
            })?;
        let basis_id = dependency
            .facts
            .basis_id
            .as_ref()
            .filter(|basis| !basis.is_empty())
            .ok_or_else(|| WcetRefusal::MissingBasis {
                dependency_id: dependency.dependency_id.clone(),
            })?;
        total_wcet_us = total_wcet_us
            .checked_add(wcet_us)
            .ok_or(WcetRefusal::WcetOverflow)?;
        total_resource_units = total_resource_units
            .checked_add(dependency.facts.resource_units)
            .ok_or(WcetRefusal::ResourceOverflow)?;
        basis_ids.push(basis_id.clone());
    }
    if total_wcet_us > region.deadline_us {
        return Err(WcetRefusal::DeadlineExceeded {
            required_us: total_wcet_us,
            deadline_us: region.deadline_us,
        });
    }
    if total_resource_units > region.maximum_resource_units {
        return Err(WcetRefusal::ResourceExceeded {
            required: total_resource_units,
            maximum: region.maximum_resource_units,
        });
    }
    Ok(DeadlineAdmission {
        region_id: region.region_id.clone(),
        deadline_us: region.deadline_us,
        maximum_resource_units: region.maximum_resource_units,
        total_wcet_us,
        total_resource_units,
        basis_ids,
    })
}

/// Check a replacement candidate without mutating the already admitted basis.
/// A replan is valid only if the replacement independently satisfies the same
/// deadline/resource contract; it may not silently trade timing for capacity.
pub fn validate_replan(
    admission: &DeadlineAdmission,
    candidate: &DeadlineRegion,
) -> Result<DeadlineAdmission, WcetRefusal> {
    if candidate.region_id != admission.region_id
        || candidate.deadline_us != admission.deadline_us
        || candidate.maximum_resource_units != admission.maximum_resource_units
    {
        return Err(WcetRefusal::ReplanViolatesAdmission);
    }
    let replacement = admit_deadline_region(candidate)?;
    if replacement.total_wcet_us > admission.deadline_us {
        return Err(WcetRefusal::ReplanViolatesAdmission);
    }
    Ok(replacement)
}

#[cfg(test)]
mod tests;
