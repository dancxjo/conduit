//! Finite in-Play selection among exact plan-lowered pool realizations.
//!
//! This layer neither plans nor schedules work. It chooses one currently
//! admissible member placement from the immutable lowered envelope, then asks
//! the existing fixed pool to perform the atomic occupation transition.

mod cost;
pub use cost::*;

use core::cmp::Ordering;

use super::{FixedSharedPool, MemberIdentity, MemberKey, MemberPlacement, PoolError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweredObservationHealth {
    Ready,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweredPoolRealization {
    pub realization: u16,
    pub placement: MemberPlacement,
    pub boot: u16,
    pub offer_generation: u64,
    pub capability: u16,
    pub implementation: u16,
    pub artifact: u16,
    pub member_capacity: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweredPoolResourceRequirement {
    pub realization: u16,
    pub resource: u16,
    pub units: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweredPoolObservation {
    pub realization: u16,
    pub resource: u16,
    pub boot: u16,
    pub offer_generation: u64,
    pub capability: u16,
    pub implementation: u16,
    pub artifact: u16,
    pub health: LoweredObservationHealth,
    pub unreserved_units: u32,
    pub utilized_units: u32,
    pub sign: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolSelectionPolicy {
    /// Compare each resource in sealed order. The first differing resource
    /// prefers greater unreserved capacity, then lower utilization. Exact ties
    /// retain Plan order.
    MoreUnreservedThenLessUtilizedThenPlanOrder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedPoolMember {
    pub member: MemberIdentity,
    pub observation_sign_count: u16,
    pub examined_realizations: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolSelectionError {
    InvalidEnvelope,
    CostInputLimit,
    DuplicateCost,
    DuplicateObservation,
    EvidenceTooSmall,
    CapacityUnavailable { examined_realizations: u16 },
    NoCurrentRealization { examined_realizations: u16 },
    Admission(PoolError),
}

/// Select and atomically occupy one exact realization for a new call.
/// At most `envelope.len()` realizations are examined; observations for
/// compatible but unsealed realizations are ignored. Selected observation Sign
/// identities are copied to caller-owned fixed storage.
#[allow(clippy::too_many_arguments)]
pub fn admit_selected_pool_member<const SLOTS: usize, const SIGN: usize>(
    pool: &mut FixedSharedPool<SLOTS, SIGN>,
    key: MemberKey,
    authority: u16,
    envelope: &[LoweredPoolRealization],
    requirements: &[LoweredPoolResourceRequirement],
    observations: &[LoweredPoolObservation],
    policy: PoolSelectionPolicy,
    selected_observation_signs: &mut [u16],
) -> Result<SelectedPoolMember, PoolSelectionError> {
    admit_pool_member(
        pool,
        key,
        authority,
        envelope,
        requirements,
        observations,
        policy,
        selected_observation_signs,
        None,
    )
}

/// Compare prepared completion costs only after the ordinary hard/current and
/// population checks. Known costs precede unavailable costs; equal or absent
/// costs use the existing capacity policy and immutable Plan order. This is a
/// new-operation selection: admitted members are never moved or replayed.
#[allow(clippy::too_many_arguments)]
pub fn admit_cost_selected_pool_member<const SLOTS: usize, const SIGN: usize>(
    pool: &mut FixedSharedPool<SLOTS, SIGN>,
    key: MemberKey,
    authority: u16,
    envelope: &[LoweredPoolRealization],
    requirements: &[LoweredPoolResourceRequirement],
    observations: &[LoweredPoolObservation],
    costs: PoolCostSelection<'_>,
    selected_observation_signs: &mut [u16],
    decisions: &mut [PoolCostCandidateDecision],
) -> Result<SelectedPoolMember, PoolSelectionError> {
    costs.validate(envelope, requirements.len(), observations.len())?;
    if decisions.len() < envelope.len() {
        return Err(PoolSelectionError::EvidenceTooSmall);
    }
    admit_pool_member(
        pool,
        key,
        authority,
        envelope,
        requirements,
        observations,
        PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        selected_observation_signs,
        Some((costs, decisions)),
    )
}

#[allow(clippy::too_many_arguments)]
fn admit_pool_member<const SLOTS: usize, const SIGN: usize>(
    pool: &mut FixedSharedPool<SLOTS, SIGN>,
    key: MemberKey,
    authority: u16,
    envelope: &[LoweredPoolRealization],
    requirements: &[LoweredPoolResourceRequirement],
    observations: &[LoweredPoolObservation],
    policy: PoolSelectionPolicy,
    selected_observation_signs: &mut [u16],
    mut cost_selection: Option<(PoolCostSelection<'_>, &mut [PoolCostCandidateDecision])>,
) -> Result<SelectedPoolMember, PoolSelectionError> {
    validate_envelope(envelope, requirements)?;
    let mut selected: Option<&LoweredPoolRealization> = None;
    let mut examined = 0_u16;
    let mut current_but_full = false;
    for (index, candidate) in envelope.iter().enumerate() {
        if let Some((_, decisions)) = &mut cost_selection {
            decisions[index] = PoolCostCandidateDecision {
                realization: candidate.realization,
                ..PoolCostCandidateDecision::EMPTY
            };
        }
        examined = examined.saturating_add(1);
        if !is_current(candidate, requirements, observations)? {
            continue;
        }
        if let Some((costs, decisions)) = &mut cost_selection {
            decisions[index].cost = costs.availability(candidate);
            decisions[index].admission = PoolCostAdmission::Eligible;
        }
        if pool.population_for_realization(candidate.realization) >= candidate.member_capacity {
            if let Some((_, decisions)) = &mut cost_selection {
                decisions[index].admission = PoolCostAdmission::Full;
            }
            current_but_full = true;
            continue;
        }
        let should_select = match selected {
            Some(current) => cost_selection
                .as_ref()
                .and_then(|(costs, _)| costs.better(candidate, current))
                .unwrap_or_else(|| better(candidate, current, requirements, observations, policy)),
            None => true,
        };
        if should_select {
            selected = Some(candidate);
        }
    }

    let Some(candidate) = selected else {
        return Err(if current_but_full {
            PoolSelectionError::CapacityUnavailable {
                examined_realizations: examined,
            }
        } else {
            PoolSelectionError::NoCurrentRealization {
                examined_realizations: examined,
            }
        });
    };
    let candidate_requirements = requirements
        .iter()
        .filter(|requirement| requirement.realization == candidate.realization);
    let required_signs = candidate_requirements.clone().count();
    if selected_observation_signs.len() < required_signs {
        return Err(PoolSelectionError::EvidenceTooSmall);
    }
    for (index, requirement) in candidate_requirements.enumerate() {
        selected_observation_signs[index] =
            exact_observation(candidate, requirement, observations)?
                .ok_or(PoolSelectionError::InvalidEnvelope)?
                .sign;
    }
    let member = pool
        .admit(key, candidate.placement, authority)
        .map_err(PoolSelectionError::Admission)?;
    Ok(SelectedPoolMember {
        member,
        observation_sign_count: required_signs as u16,
        examined_realizations: examined,
    })
}

fn validate_envelope(
    envelope: &[LoweredPoolRealization],
    requirements: &[LoweredPoolResourceRequirement],
) -> Result<(), PoolSelectionError> {
    if envelope.is_empty()
        || envelope.iter().enumerate().any(|(index, candidate)| {
            candidate.member_capacity == 0
                || candidate.realization != candidate.placement.realization
                || envelope[..index]
                    .iter()
                    .any(|prior| prior.realization == candidate.realization)
                || !requirements
                    .iter()
                    .any(|requirement| requirement.realization == candidate.realization)
        })
        || requirements.iter().enumerate().any(|(index, requirement)| {
            requirement.units == 0
                || !envelope
                    .iter()
                    .any(|candidate| candidate.realization == requirement.realization)
                || requirements[..index].iter().any(|prior| {
                    prior.realization == requirement.realization
                        && prior.resource == requirement.resource
                })
        })
    {
        Err(PoolSelectionError::InvalidEnvelope)
    } else {
        Ok(())
    }
}

fn is_current(
    candidate: &LoweredPoolRealization,
    requirements: &[LoweredPoolResourceRequirement],
    observations: &[LoweredPoolObservation],
) -> Result<bool, PoolSelectionError> {
    for requirement in requirements
        .iter()
        .filter(|requirement| requirement.realization == candidate.realization)
    {
        let Some(observation) = exact_observation(candidate, requirement, observations)? else {
            return Ok(false);
        };
        if observation.health != LoweredObservationHealth::Ready
            || observation.unreserved_units < requirement.units
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn exact_observation<'a>(
    candidate: &LoweredPoolRealization,
    requirement: &LoweredPoolResourceRequirement,
    observations: &'a [LoweredPoolObservation],
) -> Result<Option<&'a LoweredPoolObservation>, PoolSelectionError> {
    let mut exact = observations.iter().filter(|observation| {
        observation.realization == candidate.realization
            && observation.resource == requirement.resource
            && observation.boot == candidate.boot
            && observation.offer_generation == candidate.offer_generation
            && observation.capability == candidate.capability
            && observation.implementation == candidate.implementation
            && observation.artifact == candidate.artifact
    });
    let first = exact.next();
    if exact.next().is_some() {
        Err(PoolSelectionError::DuplicateObservation)
    } else {
        Ok(first)
    }
}

fn better(
    candidate: &LoweredPoolRealization,
    current: &LoweredPoolRealization,
    requirements: &[LoweredPoolResourceRequirement],
    observations: &[LoweredPoolObservation],
    policy: PoolSelectionPolicy,
) -> bool {
    match policy {
        PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder => requirements
            .iter()
            .filter(|requirement| requirement.realization == candidate.realization)
            .zip(
                requirements
                    .iter()
                    .filter(|requirement| requirement.realization == current.realization),
            )
            .find_map(|(candidate_requirement, current_requirement)| {
                let candidate_observation =
                    exact_observation(candidate, candidate_requirement, observations).ok()??;
                let current_observation =
                    exact_observation(current, current_requirement, observations).ok()??;
                match candidate_observation
                    .unreserved_units
                    .cmp(&current_observation.unreserved_units)
                {
                    Ordering::Greater => Some(true),
                    Ordering::Less => Some(false),
                    Ordering::Equal => match candidate_observation
                        .utilized_units
                        .cmp(&current_observation.utilized_units)
                    {
                        Ordering::Less => Some(true),
                        Ordering::Greater => Some(false),
                        Ordering::Equal => None,
                    },
                }
            })
            .unwrap_or(false),
    }
}
