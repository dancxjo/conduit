//! Finite in-Play selection among exact plan-lowered pool realizations.
//!
//! This layer neither plans nor schedules work. It chooses one currently
//! admissible member placement from the immutable lowered envelope, then asks
//! the existing fixed pool to perform the atomic occupation transition.

use super::{FixedSharedPool, MemberIdentity, MemberKey, MemberPlacement, PoolError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweredObservationHealth {
    Ready,
    Unavailable,
}

/// Exact stable identity and finite capacity sealed into one immutable Plan.
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
    pub required_units: u32,
}

/// One current, attributable observation lowered before it reaches the kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweredPoolObservation {
    pub realization: u16,
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
    MoreUnreservedThenLessUtilizedThenPlanOrder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedPoolMember {
    pub member: MemberIdentity,
    pub observation_sign: u16,
    pub examined_realizations: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolSelectionError {
    InvalidEnvelope,
    DuplicateObservation,
    NoCurrentRealization { examined_realizations: u16 },
    Admission(PoolError),
}

/// Select and atomically occupy one exact realization for a new operation.
/// At most `envelope.len()` realizations are examined; observations for
/// compatible but unsealed realizations are ignored.
pub fn admit_selected_pool_member<const SLOTS: usize, const SIGN: usize>(
    pool: &mut FixedSharedPool<SLOTS, SIGN>,
    key: MemberKey,
    authority: u16,
    envelope: &[LoweredPoolRealization],
    observations: &[LoweredPoolObservation],
    policy: PoolSelectionPolicy,
) -> Result<SelectedPoolMember, PoolSelectionError> {
    if envelope.is_empty()
        || envelope.iter().enumerate().any(|(index, candidate)| {
            candidate.member_capacity == 0
                || candidate.required_units == 0
                || candidate.realization != candidate.placement.realization
                || envelope[..index]
                    .iter()
                    .any(|prior| prior.realization == candidate.realization)
        })
    {
        return Err(PoolSelectionError::InvalidEnvelope);
    }

    let mut selected: Option<(&LoweredPoolRealization, &LoweredPoolObservation)> = None;
    let mut examined = 0_u16;
    for candidate in envelope {
        examined = examined.saturating_add(1);
        if pool.population_for_realization(candidate.realization) >= candidate.member_capacity {
            continue;
        }
        let mut exact = observations.iter().filter(|observation| {
            observation.realization == candidate.realization
                && observation.boot == candidate.boot
                && observation.offer_generation == candidate.offer_generation
                && observation.capability == candidate.capability
                && observation.implementation == candidate.implementation
                && observation.artifact == candidate.artifact
        });
        let Some(observation) = exact.next() else {
            continue;
        };
        if exact.next().is_some() {
            return Err(PoolSelectionError::DuplicateObservation);
        }
        if observation.health != LoweredObservationHealth::Ready
            || observation.unreserved_units < candidate.required_units
        {
            continue;
        }
        let replace = match (policy, selected) {
            (_, None) => true,
            (
                PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
                Some((_, current)),
            ) => {
                observation.unreserved_units > current.unreserved_units
                    || (observation.unreserved_units == current.unreserved_units
                        && observation.utilized_units < current.utilized_units)
            }
        };
        if replace {
            selected = Some((candidate, observation));
        }
    }

    let Some((candidate, observation)) = selected else {
        return Err(PoolSelectionError::NoCurrentRealization {
            examined_realizations: examined,
        });
    };
    let member = pool
        .admit(key, candidate.placement, authority)
        .map_err(PoolSelectionError::Admission)?;
    Ok(SelectedPoolMember {
        member,
        observation_sign: observation.sign,
        examined_realizations: examined,
    })
}
