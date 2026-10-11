//! Cost preference over the ordinary Planner's admitted candidate records.
use super::{
    CostCalibrationRevision, CostDemand, CostEvidenceRefusal as Refusal, CostObservationWindow,
    CostScopeChange, CostedForecast,
};
use crate::{
    HardRealizationRequirements, PerformanceIntent, PlannerError, RealizationDecisionDisposition,
    RealizationPolicy, RealizationSelection,
};
use alloc::vec::Vec;
use conduit_core::{
    CapabilityId, ExactNumericWork, HostAdvertisement, HostId, MonotonicInstant,
    RealizationAdvertisement, ResourceObservation,
};
use core::cmp::Ordering;

pub const MAXIMUM_COST_CANDIDATES: usize = 32;
/// Current clock and reviewed calibration catalogue supplied by the planning
/// boundary, independently of the sampled producer evidence.
pub struct CostEvaluationContext<'a> {
    pub host: &'a HostAdvertisement,
    pub calibrations: &'a [CostCalibrationRevision<'a>],
    pub now: &'a MonotonicInstant,
}
pub struct TimedResourceObservation<'a> {
    observation: &'a ResourceObservation,
    window: CostObservationWindow,
}
impl<'a> TimedResourceObservation<'a> {
    pub fn admit(
        observation: &'a ResourceObservation,
        window: CostObservationWindow,
    ) -> Result<Self, Refusal> {
        if window.observed().clock().host_id() != &observation.host_id
            || window.observed().clock().boot_id() != &observation.boot_id
        {
            return Err(Refusal::UnadvertisedSubject);
        }
        Ok(Self {
            observation,
            window,
        })
    }
    pub fn observation(&self) -> &'a ResourceObservation {
        self.observation
    }
    pub fn window(&self) -> &CostObservationWindow {
        &self.window
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableCostPolicy {
    PreferKnown,
    IgnoreCostsIfIncomplete,
    RequireKnown,
}
pub struct CostSelectionInputs<'a> {
    pub hosts: &'a [HostAdvertisement],
    pub contexts: &'a [CostEvaluationContext<'a>],
    pub advertisements: &'a [RealizationAdvertisement],
    pub requirements: &'a HardRealizationRequirements,
    pub observations: &'a [TimedResourceObservation<'a>],
    pub tie_policy: &'a RealizationPolicy,
    pub forecasts: &'a [CostedForecast<'a>],
    pub intent: PerformanceIntent,
    pub unavailable: UnavailableCostPolicy,
}
#[derive(Debug)]
pub enum CostAvailability<'a> {
    Known(&'a CostedForecast<'a>),
    Missing,
    Unusable(Refusal),
}
#[derive(Debug)]
pub struct CostCandidateEvidence<'a> {
    pub host_id: &'a HostId,
    pub capability_id: &'a CapabilityId,
    pub availability: CostAvailability<'a>,
    pub selected: bool,
}
#[derive(Debug)]
pub struct CostRealizationSelection<'a> {
    /// All original hard refusals are retained. Exactly one admitted record is
    /// selected after cost preference; costs never change a rejected record.
    pub realization: RealizationSelection,
    pub intent: PerformanceIntent,
    pub costs: Vec<CostCandidateEvidence<'a>>,
    pub used_cost_preference: bool,
    /// Conservative admitted limb work, not empirical scheduling wall time.
    pub arithmetic_work_spent: u64,
}
#[derive(Debug)]
pub struct UnavailableCostEvidence {
    pub host_id: HostId,
    pub capability_id: CapabilityId,
    /// None denotes missing evidence, not a numeric zero.
    pub reason: Option<Refusal>,
}
#[derive(Debug)]
pub enum CostSelectionError {
    Planner(PlannerError),
    Evidence(Refusal),
    NoUsableForecast {
        realization: RealizationSelection,
        costs: Vec<UnavailableCostEvidence>,
    },
}
impl From<PlannerError> for CostSelectionError {
    fn from(value: PlannerError) -> Self {
        Self::Planner(value)
    }
}
impl From<Refusal> for CostSelectionError {
    fn from(value: Refusal) -> Self {
        Self::Evidence(value)
    }
}

/// Finite operation fulfillment preference for the existing PerformanceIntent.
/// Interactive, streaming-prefix, batch and background compare the upper model
/// estimate for this exact demand. BoundedResponse requires certified evidence
/// outside this estimated-cost API and is explicitly refused.
pub fn select_realization_with_costs<'a>(
    demand: CostDemand<'a>,
    inputs: CostSelectionInputs<'a>,
    work: &mut ExactNumericWork,
) -> Result<CostRealizationSelection<'a>, CostSelectionError> {
    let initial_work = work.spent();
    if inputs.contexts.is_empty()
        || inputs.contexts.len() > MAXIMUM_COST_CANDIDATES
        || inputs.contexts.len() != inputs.hosts.len()
        || inputs.forecasts.len() > MAXIMUM_COST_CANDIDATES
        || inputs.advertisements.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
        || inputs.observations.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
        || inputs
            .hosts
            .iter()
            .map(|host| host.capabilities.len())
            .sum::<usize>()
            > MAXIMUM_COST_CANDIDATES
    {
        return Err(Refusal::InputLimit.into());
    }
    for (index, context) in inputs.contexts.iter().enumerate() {
        if !inputs.hosts.iter().any(|host| host == context.host) {
            return Err(Refusal::UnadvertisedSubject.into());
        }
        for (position, offer) in context.host.capabilities.iter().enumerate() {
            if context.host.capabilities[..position]
                .iter()
                .any(|other| other.capability_id == offer.capability_id)
            {
                return Err(Refusal::AmbiguousEvidence.into());
            }
        }
        if inputs.contexts[..index]
            .iter()
            .any(|previous| previous.host.host_id == context.host.host_id)
            || context.calibrations.len() > MAXIMUM_COST_CANDIDATES
        {
            return Err(Refusal::AmbiguousEvidence.into());
        }
        context.now.validate().map_err(Refusal::InvalidClock)?;
        if context.now.clock().host_id() != &context.host.host_id
            || context.now.clock().boot_id() != &context.host.boot_id
        {
            return Err(Refusal::UnadvertisedSubject.into());
        }
        for (position, calibration) in context.calibrations.iter().enumerate() {
            if context.calibrations[..position]
                .iter()
                .any(|other| other.identity() == calibration.identity())
            {
                return Err(Refusal::AmbiguousEvidence.into());
            }
        }
    }
    let mut observations = Vec::with_capacity(inputs.observations.len());
    for timed in inputs.observations {
        let context = inputs
            .contexts
            .iter()
            .find(|context| context.host.host_id == timed.observation.host_id)
            .ok_or(Refusal::UnadvertisedSubject)?;
        timed.window.validate_at(context.now)?;
        observations.push(timed.observation.clone());
    }
    // This is the existing generic hard-admission/health/predicate evaluator,
    // including its original explanations, not an independent scheduler.
    let mut realization = crate::select_realization_with_characteristics_and_signs(
        demand.class().gear(),
        inputs.hosts,
        inputs.advertisements,
        inputs.requirements,
        &observations,
        inputs.tie_policy,
    )?;
    if inputs.intent == PerformanceIntent::BoundedResponse {
        return Err(Refusal::UnsupportedIntent.into());
    }
    for (index, forecast) in inputs.forecasts.iter().enumerate() {
        if inputs.forecasts[..index].iter().any(|other| {
            other.source().host().host_id == forecast.source().host().host_id
                && other.source().offer().capability_id == forecast.source().offer().capability_id
        }) {
            return Err(Refusal::AmbiguousEvidence.into());
        }
    }
    let mut costs = Vec::new();
    for record in &realization.signs {
        if matches!(
            record.disposition,
            RealizationDecisionDisposition::Rejected(_)
        ) {
            continue;
        }
        let context = inputs
            .contexts
            .iter()
            .find(|context| context.host.host_id == record.host_id)
            .expect("record names admitted Host");
        let offer = context
            .host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == record.capability_id)
            .expect("record names admitted offer");
        let availability = if let Some(forecast) = inputs.forecasts.iter().find(|forecast| {
            forecast.source().host().host_id == record.host_id
                && forecast.source().offer().capability_id == record.capability_id
        }) {
            let validation = forecast.demand().validate_match(demand).and_then(|()| {
                let calibration = context
                    .calibrations
                    .iter()
                    .find(|revision| {
                        revision.identity() == forecast.source().calibration().identity()
                    })
                    .ok_or(Refusal::Scope(CostScopeChange::Calibration))?;
                forecast.validate_at(context.host, *calibration, context.now)
            });
            match validation {
                Ok(()) => CostAvailability::Known(forecast),
                Err(error) => CostAvailability::Unusable(error),
            }
        } else {
            CostAvailability::Missing
        };
        costs.push(CostCandidateEvidence {
            host_id: &context.host.host_id,
            capability_id: &offer.capability_id,
            availability,
            selected: false,
        });
    }
    let complete = costs
        .iter()
        .all(|item| matches!(item.availability, CostAvailability::Known(_)));
    let use_cost = inputs.unavailable != UnavailableCostPolicy::IgnoreCostsIfIncomplete || complete;
    let mut winner: Option<usize> = None;
    if use_cost {
        // Fallible linear comparison spends one explicit shared arithmetic
        // budget. No panic-on-sort comparator or rounded scalar ranking key.
        for (index, candidate) in costs.iter().enumerate() {
            let CostAvailability::Known(forecast) = &candidate.availability else {
                continue;
            };
            let better = if let Some(previous) = winner {
                let CostAvailability::Known(previous_forecast) = &costs[previous].availability
                else {
                    unreachable!()
                };
                let order = forecast
                    .upper_seconds()
                    .checked_compare(previous_forecast.upper_seconds(), work)
                    .map_err(Refusal::from)?;
                order == Ordering::Less
                    || (order == Ordering::Equal
                        && tie_order(candidate, &costs[previous], &inputs, &observations)
                            == Ordering::Less)
            } else {
                true
            };
            if better {
                winner = Some(index);
            }
        }
    }
    if winner.is_none() && inputs.unavailable == UnavailableCostPolicy::RequireKnown {
        let costs = costs
            .into_iter()
            .map(|item| UnavailableCostEvidence {
                host_id: item.host_id.clone(),
                capability_id: item.capability_id.clone(),
                reason: match item.availability {
                    CostAvailability::Missing => None,
                    CostAvailability::Unusable(reason) => Some(reason),
                    CostAvailability::Known(_) => {
                        unreachable!("known candidate would have a winner")
                    }
                },
            })
            .collect();
        return Err(CostSelectionError::NoUsableForecast { realization, costs });
    }
    if let Some(index) = winner {
        realization.choice.host_id = costs[index].host_id.clone();
        realization.choice.capability_id = costs[index].capability_id.clone();
        for record in &mut realization.signs {
            if !matches!(
                record.disposition,
                RealizationDecisionDisposition::Rejected(_)
            ) {
                record.disposition = if record.host_id == realization.choice.host_id
                    && record.capability_id == realization.choice.capability_id
                {
                    RealizationDecisionDisposition::Selected
                } else {
                    RealizationDecisionDisposition::Admitted
                };
                record.decisive_preference_clause = None;
            }
        }
    }
    for record in &mut costs {
        record.selected = record.host_id == &realization.choice.host_id
            && record.capability_id == &realization.choice.capability_id;
    }
    Ok(CostRealizationSelection {
        realization,
        intent: inputs.intent,
        costs,
        used_cost_preference: winner.is_some(),
        arithmetic_work_spent: work.spent() - initial_work,
    })
}
fn tie_order(
    left: &CostCandidateEvidence<'_>,
    right: &CostCandidateEvidence<'_>,
    inputs: &CostSelectionInputs<'_>,
    observations: &[ResourceObservation],
) -> Ordering {
    let subject = |host_id: &HostId, capability_id: &CapabilityId| {
        let host = inputs
            .contexts
            .iter()
            .find(|context| &context.host.host_id == host_id)
            .expect("admitted host")
            .host;
        let offer = host
            .capabilities
            .iter()
            .find(|offer| &offer.capability_id == capability_id)
            .expect("admitted offer");
        let advertisement = inputs.advertisements.iter().find(|item| {
            item.host_id == host.host_id
                && item.boot_id == host.boot_id
                && item.offer_generation == host.offer_generation
                && item.capability_id == offer.capability_id
        });
        (host, offer, advertisement)
    };
    let (left_host, left_offer, left_ad) = subject(left.host_id, left.capability_id);
    let (right_host, right_offer, right_ad) = subject(right.host_id, right.capability_id);
    crate::characteristic_policy::compare(
        left_host,
        left_offer,
        left_ad,
        right_host,
        right_offer,
        right_ad,
        observations,
        inputs.tie_policy,
    )
}
