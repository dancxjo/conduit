//! Cost preference over the ordinary Planner's admitted candidate records.
use super::selection_account::*;
use super::{
    CostCalibrationRevision, CostDemand, CostEvidenceRefusal as Refusal, CostObservationWindow,
    CostScopeChange, CostedForecast,
};
use crate::{
    HardRealizationRequirements, PerformanceIntent, RealizationDecisionDisposition,
    RealizationPolicy,
};
use alloc::vec::Vec;
use conduit_core::{
    CapabilityId, ExactNumericWork, HostAdvertisement, HostId, MonotonicInstant,
    RealizationAdvertisement, ResourceObservation,
};
use core::cmp::Ordering;

pub const MAXIMUM_COST_CANDIDATES: usize = 32;
/// Each complete owner inventory is scanned independently of compatible cost
/// comparands. At most 32 Hosts with 256 Offers each enter this preparation.
pub const MAXIMUM_COST_OFFERS_PER_HOST: usize = 256;
/// Current clock and reviewed calibration catalogue supplied by the planning
/// boundary, independently of the sampled producer evidence.
pub struct CostEvaluationContext<'a> {
    pub host: &'a HostAdvertisement,
    pub calibrations: &'a [CostCalibrationRevision<'a>],
    pub now: &'a MonotonicInstant,
    pub execution_observations: &'a [super::CostExecutionObservation<'a>],
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
pub struct CostSelectionInputs<'a, 'selection> {
    pub hosts: &'a [HostAdvertisement],
    pub contexts: &'a [CostEvaluationContext<'a>],
    pub advertisements: &'selection [RealizationAdvertisement],
    pub requirements: &'selection HardRealizationRequirements,
    pub observations: &'selection [TimedResourceObservation<'selection>],
    pub tie_policy: &'selection RealizationPolicy,
    pub forecasts: &'a [CostedForecast<'a>],
    pub intent: PerformanceIntent,
    pub unavailable: UnavailableCostPolicy,
}
/// Finite operation fulfillment preference for the existing PerformanceIntent.
/// Interactive, streaming-prefix, batch and background compare the upper model
/// estimate for this exact demand. BoundedResponse requires certified evidence
/// outside this estimated-cost API and is explicitly refused.
pub fn select_realization_with_costs<'a>(
    demand: CostDemand<'a>,
    inputs: CostSelectionInputs<'a, '_>,
    work: &mut ExactNumericWork,
) -> Result<CostRealizationSelection<'a>, CostSelectionError> {
    let initial_work = work.spent();
    let initial_budget = work.remaining();
    if inputs.contexts.is_empty()
        || inputs.contexts.len() > MAXIMUM_COST_CANDIDATES
        || inputs.contexts.len() != inputs.hosts.len()
        || inputs.forecasts.len() > MAXIMUM_COST_CANDIDATES
        || inputs.advertisements.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
        || inputs.observations.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS
        || inputs
            .hosts
            .iter()
            .any(|host| host.capabilities.len() > MAXIMUM_COST_OFFERS_PER_HOST)
    {
        return Err(Refusal::InputLimit.into());
    }
    for (index, context) in inputs.contexts.iter().enumerate() {
        if !inputs.hosts.iter().any(|host| host == context.host) {
            return Err(Refusal::UnadvertisedSubject.into());
        }
        // Bound already checked above and subject equality pins this inventory.
        // Sorting borrowed IDs uses fixed scratch and creates no heap storage.
        let mut ids = [None; MAXIMUM_COST_OFFERS_PER_HOST];
        let count = context.host.capabilities.len();
        for (slot, offer) in ids.iter_mut().zip(&context.host.capabilities) {
            *slot = Some(&offer.capability_id);
        }
        ids[..count].sort_unstable();
        if ids[..count].windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Refusal::AmbiguousEvidence.into());
        }
        if inputs.contexts[..index]
            .iter()
            .any(|previous| previous.host.host_id == context.host.host_id)
            || context.calibrations.len() > MAXIMUM_COST_CANDIDATES
        {
            return Err(Refusal::AmbiguousEvidence.into());
        }
        if context.execution_observations.len() > MAXIMUM_COST_CANDIDATES {
            return Err(Refusal::InputLimit.into());
        }
        for (position, observation) in context.execution_observations.iter().enumerate() {
            if observation.source().host().host_id != context.host.host_id {
                return Err(Refusal::UnadvertisedSubject.into());
            }
            if context.execution_observations[..position]
                .iter()
                .any(|other| {
                    other.source().offer().capability_id
                        == observation.source().offer().capability_id
                })
            {
                return Err(Refusal::AmbiguousEvidence.into());
            }
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
    let mut requirements = inputs.requirements.clone();
    if !demand.inputs().is_empty() {
        requirements.minimum_queue_items = requirements.minimum_queue_items.max(1);
        let largest = demand
            .inputs()
            .iter()
            .map(|input| input.value.encoded.len())
            .max()
            .unwrap_or(0);
        // The admitted aggregate input bound is below u32::MAX. A realization
        // must at least hold one complete input; a cheap forecast cannot admit
        // a payload larger than its queue's entire byte extent.
        requirements.minimum_queue_bytes = requirements.minimum_queue_bytes.max(largest as u32);
    }
    let mut realization = crate::select_realization_with_characteristics_account(
        demand.class().gear(),
        inputs.hosts,
        inputs.advertisements,
        &requirements,
        &observations,
        inputs.tie_policy,
    )
    .map_err(CostSelectionError::HardAdmission)?;
    let admitted = realization
        .signs
        .iter()
        .filter(|record| {
            !matches!(
                record.disposition,
                RealizationDecisionDisposition::Rejected(_)
            )
        })
        .count();
    if admitted > MAXIMUM_COST_CANDIDATES {
        return Err(policy_refusal(
            realization,
            inputs.intent,
            Refusal::InputLimit,
        ));
    }
    if inputs.intent == PerformanceIntent::BoundedResponse {
        return Err(policy_refusal(
            realization,
            inputs.intent,
            Refusal::UnsupportedIntent,
        ));
    }
    for (index, forecast) in inputs.forecasts.iter().enumerate() {
        if inputs.forecasts[..index].iter().any(|other| {
            other.source().host().host_id == forecast.source().host().host_id
                && other.source().offer().capability_id == forecast.source().offer().capability_id
        }) {
            return Err(policy_refusal(
                realization,
                inputs.intent,
                Refusal::AmbiguousEvidence,
            ));
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
        let submitted = inputs.forecasts.iter().find(|forecast| {
            forecast.source().host().host_id == record.host_id
                && forecast.source().offer().capability_id == record.capability_id
        });
        let current_execution = context
            .execution_observations
            .iter()
            .find(|observation| observation.source().offer().capability_id == record.capability_id);
        let availability = if let Some(forecast) = submitted {
            let validation = forecast.demand().validate_match(demand).and_then(|()| {
                let calibration = context
                    .calibrations
                    .iter()
                    .find(|revision| {
                        revision.identity() == forecast.source().calibration().identity()
                    })
                    .ok_or(Refusal::Scope(CostScopeChange::Calibration))?;
                forecast.validate_for_execution(
                    context.host,
                    *calibration,
                    context.now,
                    current_execution,
                )
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
            submitted_source: submitted.map(|sample| sample.source()),
            submitted_calibration_source: submitted
                .and_then(|sample| sample.calibration().map(|estimate| estimate.source())),
            evaluated_at: context.now,
            current_execution,
            submitted_execution_assumption: submitted.and_then(|sample| {
                sample
                    .execution_assumption()
                    .map(|observation| observation.source())
            }),
            expected_execution_condition: submitted
                .and_then(|sample| sample.expected_execution_condition()),
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
                let order = match forecast
                    .upper_seconds()
                    .checked_compare(previous_forecast.upper_seconds(), work)
                {
                    Ok(order) => order,
                    Err(reason) => {
                        return Err(CostSelectionError::Comparison(alloc::boxed::Box::new(
                            CostComparisonRefusal {
                                operands: [
                                    CostComparisonOperand::retain(candidate, forecast),
                                    CostComparisonOperand::retain(
                                        &costs[previous],
                                        previous_forecast,
                                    ),
                                ],
                                realization,
                                reason: Refusal::from(reason),
                                intent: inputs.intent,
                                arithmetic_work_limit: initial_budget,
                                arithmetic_work_spent: work.spent() - initial_work,
                                arithmetic_work_remaining: work.remaining(),
                            },
                        )))
                    }
                };
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
                submitted: item
                    .submitted_source
                    .map(super::CostSourceAttribution::from_source),
                submitted_calibration: item
                    .submitted_calibration_source
                    .map(super::CostSourceAttribution::from_source),
                current_execution: item.current_execution.map(|observation| {
                    (
                        super::CostSourceAttribution::from_source(observation.source()),
                        observation.condition(),
                    )
                }),
                submitted_execution_assumption: item
                    .submitted_execution_assumption
                    .map(super::CostSourceAttribution::from_source),
                expected_execution_condition: item.expected_execution_condition,
                evaluated_at: item.evaluated_at.clone(),
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
    inputs: &CostSelectionInputs<'_, '_>,
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

fn policy_refusal(
    realization: crate::RealizationSelection,
    intent: PerformanceIntent,
    reason: Refusal,
) -> CostSelectionError {
    CostSelectionError::Policy(alloc::boxed::Box::new(CostPolicyRefusal {
        realization,
        intent,
        reason,
    }))
}
