//! Bounded, local reluctant-replanning decisions outside Play.
//! A recommendation never installs a Plan or grants execution authority.
use super::{
    CostDuration, CostEvaluationContext, CostEvidenceRefusal, CostMeasuredDuration,
    CostSourceAttribution, CostedForecast, ReplacementCostFacts, ReplacementCostRefusal,
};
use alloc::vec::Vec;
use conduit_core::{ExactNumericWork, ExactRational, MonotonicInstant, PlanId};
use core::cmp::Ordering;

pub const MAXIMUM_REPLAN_OBSERVATIONS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReluctantReplanRefusal {
    Evidence(CostEvidenceRefusal),
    Replacement(ReplacementCostRefusal),
    InvalidPolicy,
    Capacity,
    ScopeChanged,
    ObservationOrder,
    DuplicateSign,
    SamePlan,
}
impl From<CostEvidenceRefusal> for ReluctantReplanRefusal {
    fn from(value: CostEvidenceRefusal) -> Self {
        Self::Evidence(value)
    }
}
impl From<conduit_core::ExactNumericRefusal> for ReluctantReplanRefusal {
    fn from(value: conduit_core::ExactNumericRefusal) -> Self {
        Self::Evidence(value.into())
    }
}
impl From<ReplacementCostRefusal> for ReluctantReplanRefusal {
    fn from(value: ReplacementCostRefusal) -> Self {
        Self::Replacement(value)
    }
}

/// A finite horizon of equal exact demands. This is a forecast horizon, not
/// permission to execute or replay those operations.
#[derive(Debug, Clone, Copy)]
pub struct ReluctantReplanPolicy {
    minimum_observations: usize,
    minimum_span: CostDuration,
    maximum_window: CostDuration,
    maximum_gap: CostDuration,
    cooldown: CostDuration,
    minimum_net_savings: CostDuration,
    horizon_operations: u16,
}
impl ReluctantReplanPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        minimum_observations: usize,
        minimum_span: CostDuration,
        maximum_window: CostDuration,
        maximum_gap: CostDuration,
        cooldown: CostDuration,
        minimum_net_savings: CostDuration,
        horizon_operations: u16,
        work: &mut ExactNumericWork,
    ) -> Result<Self, ReluctantReplanRefusal> {
        let positive = |v: CostDuration| v.seconds() != ExactRational::ZERO;
        if !(2..=MAXIMUM_REPLAN_OBSERVATIONS).contains(&minimum_observations)
            || horizon_operations == 0
            || horizon_operations > 1024
            || ![
                minimum_span,
                maximum_window,
                maximum_gap,
                cooldown,
                minimum_net_savings,
            ]
            .into_iter()
            .all(positive)
            || minimum_span.compare(maximum_window, work)? == Ordering::Greater
            || maximum_gap.compare(maximum_window, work)? == Ordering::Greater
        {
            return Err(ReluctantReplanRefusal::InvalidPolicy);
        }
        Ok(Self {
            minimum_observations,
            minimum_span,
            maximum_window,
            maximum_gap,
            cooldown,
            minimum_net_savings,
            horizon_operations,
        })
    }
}

pub struct ReplanCostObservation<'a> {
    pub previous: &'a CostedForecast<'a>,
    pub candidate: &'a CostedForecast<'a>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepCurrentPlanReason {
    InsufficientPersistence,
    NetBenefitNotPersistent,
    ObservationGap,
    Cooldown,
    AlreadyRecommended,
}
#[derive(Debug)]
pub struct ReplanObservationAccount {
    pub previous: CostSourceAttribution,
    pub candidate: CostSourceAttribution,
    /// Old lower fulfillment bound minus candidate upper, times finite horizon,
    /// minus upper replacement overhead. Negative values remain explicit.
    pub conservative_net_seconds: ExactRational,
}
#[derive(Debug)]
pub struct ReluctantReplanDecision {
    pub policy: ReluctantReplanPolicy,
    pub previous_recommendation: Option<(PlanId, MonotonicInstant)>,
    pub previous_plan_id: PlanId,
    pub candidate_plan_id: PlanId,
    pub evaluated_at: MonotonicInstant,
    pub keep_current: Option<KeepCurrentPlanReason>,
    pub observations: Vec<ReplanObservationAccount>,
    pub replacement_sources: [CostSourceAttribution; 5],
    pub replacement_upper_seconds: ExactRational,
    pub arithmetic_work_spent: u64,
}
impl ReluctantReplanDecision {
    pub fn recommends_replacement(&self) -> bool {
        self.keep_current.is_none()
    }
}

/// One explicit local decision owner. State is finite: last evaluation and one
/// recommendation latch. The caller retains returned decision evidence. A new
/// owner does not inherit this owner's cooldown or claim historical continuity.
#[derive(Default)]
pub struct ReluctantReplanGate {
    last_evaluation: Option<MonotonicInstant>,
    last_recommendation: Option<(PlanId, MonotonicInstant)>,
}
impl ReluctantReplanGate {
    pub fn evaluate(
        &mut self,
        policy: ReluctantReplanPolicy,
        costs: &ReplacementCostFacts<'_>,
        observations: &[ReplanCostObservation<'_>],
        context: &CostEvaluationContext<'_>,
        work: &mut ExactNumericWork,
    ) -> Result<ReluctantReplanDecision, ReluctantReplanRefusal> {
        use ReluctantReplanRefusal as R;
        if observations.is_empty() || observations.len() > MAXIMUM_REPLAN_OBSERVATIONS {
            return Err(R::Capacity);
        }
        if costs.previous().plan_id == costs.candidate().plan_id {
            return Err(R::SamePlan);
        }
        let spent = work.spent();
        let (old_gear, new_gear) = costs.gears();
        // Recheck every measured stage and actual live residence now. An old
        // admitted comparison is not perpetual freshness or resource custody.
        ReplacementCostFacts::admit(
            costs.previous(),
            old_gear,
            costs.candidate(),
            new_gear,
            core::array::from_fn(|i| Some(costs.components()[i])),
            costs.residences(),
            core::slice::from_ref(context),
            work,
        )?;
        if let Some(last) = &self.last_evaluation {
            if context.now.clock() != last.clock() || context.now.ticks() <= last.ticks() {
                return Err(R::ObservationOrder);
            }
        }
        let mut accounts = Vec::with_capacity(observations.len());
        let mut previous_tick = None;
        let mut first = None;
        let mut latest = None;
        let mut gap = false;
        let mut profitable = true;
        for observation in observations {
            let old = observation.previous;
            let new = observation.candidate;
            old.demand().validate_match(new.demand())?;
            old.demand()
                .validate_match(observations[0].previous.demand())?;
            for (forecast, gear) in [(old, old_gear), (new, new_gear)] {
                if !super::residence::matches_placement_source(gear, forecast.source())
                    || forecast.demand().class().gear().gear_id != gear.gear_id
                    || forecast.demand().class().gear().semantic_contract != gear.semantic_contract
                    || forecast.demand().class().gear().configuration != gear.configuration
                    || forecast.demand().class().gear().inputs != gear.inputs
                    || forecast.demand().class().gear().outputs != gear.outputs
                {
                    return Err(R::ScopeChanged);
                }
                validate_forecast(forecast, context)?;
                if forecast.source().window().observed().clock() != context.now.clock() {
                    return Err(R::ScopeChanged);
                }
                let sign = forecast.source().sign();
                if costs.components().iter().any(|c| c.source().sign() == sign)
                    || accounts.iter().any(|a: &ReplanObservationAccount| {
                        &a.previous.sign_id == sign || &a.candidate.sign_id == sign
                    })
                {
                    return Err(R::DuplicateSign);
                }
            }
            if old.source().sign() == new.source().sign() {
                return Err(R::DuplicateSign);
            }
            // Paired producer observations must share one sampled instant;
            // otherwise apparent savings can compare different load epochs.
            let instant = old.source().window().observed();
            if new.source().window().observed() != instant {
                return Err(R::ObservationOrder);
            }
            if let Some(prior) = previous_tick {
                if instant.ticks() <= prior {
                    return Err(R::ObservationOrder);
                }
            }
            if let Some(prior) = latest {
                if elapsed(instant, prior, work)?
                    .upper()
                    .compare(policy.maximum_gap, work)?
                    == Ordering::Greater
                {
                    gap = true;
                }
            }
            first.get_or_insert(instant);
            latest = Some(instant);
            previous_tick = Some(instant.ticks());
            let horizon = ExactRational::ratio(policy.horizon_operations.into(), 1, work)?;
            let net = old
                .lower_seconds()
                .checked_subtract(new.upper_seconds(), work)?
                .checked_multiply(horizon, work)?
                .checked_subtract(costs.sequential_overhead().upper().seconds(), work)?;
            profitable &= net.checked_compare(policy.minimum_net_savings.seconds(), work)?
                == Ordering::Greater;
            accounts.push(ReplanObservationAccount {
                previous: CostSourceAttribution::from_source(old.source()),
                candidate: CostSourceAttribution::from_source(new.source()),
                conservative_net_seconds: net,
            });
        }
        let first = first.expect("nonempty observations");
        let latest = latest.expect("nonempty observations");
        if elapsed(context.now, first, work)?
            .upper()
            .compare(policy.maximum_window, work)?
            == Ordering::Greater
        {
            return Err(R::Evidence(CostEvidenceRefusal::Stale));
        }
        if elapsed(context.now, latest, work)?
            .upper()
            .compare(policy.maximum_gap, work)?
            == Ordering::Greater
        {
            gap = true;
        }
        let persistent = observations.len() >= policy.minimum_observations
            && elapsed(latest, first, work)?
                .lower()
                .compare(policy.minimum_span, work)?
                != Ordering::Less;
        let mut reason = if gap {
            Some(KeepCurrentPlanReason::ObservationGap)
        } else if !profitable {
            Some(KeepCurrentPlanReason::NetBenefitNotPersistent)
        } else if !persistent {
            Some(KeepCurrentPlanReason::InsufficientPersistence)
        } else {
            None
        };
        if let Some((plan, last)) = &self.last_recommendation {
            if plan == &costs.previous().plan_id {
                reason = Some(KeepCurrentPlanReason::AlreadyRecommended);
            } else if elapsed(context.now, last, work)?
                .lower()
                .compare(policy.cooldown, work)?
                == Ordering::Less
            {
                reason = Some(KeepCurrentPlanReason::Cooldown);
            }
        }
        // Failures above leave both latches untouched. A recommendation latches
        // this old Plan even when the caller has not yet installed its successor.
        let previous_recommendation = self.last_recommendation.clone();
        self.last_evaluation = Some(context.now.clone());
        if reason.is_none() {
            self.last_recommendation =
                Some((costs.previous().plan_id.clone(), context.now.clone()));
        }
        Ok(ReluctantReplanDecision {
            policy,
            previous_recommendation,
            previous_plan_id: costs.previous().plan_id.clone(),
            candidate_plan_id: costs.candidate().plan_id.clone(),
            evaluated_at: context.now.clone(),
            keep_current: reason,
            observations: accounts,
            replacement_sources: core::array::from_fn(|i| {
                CostSourceAttribution::from_source(costs.components()[i].source())
            }),
            replacement_upper_seconds: costs.sequential_overhead().upper().seconds(),
            arithmetic_work_spent: work.spent() - spent,
        })
    }
}
fn elapsed(
    end: &MonotonicInstant,
    start: &MonotonicInstant,
    work: &mut ExactNumericWork,
) -> Result<super::CostDurationRange, ReluctantReplanRefusal> {
    Ok(CostMeasuredDuration::measure(start, end, work)?.range())
}
fn validate_forecast(
    forecast: &CostedForecast<'_>,
    context: &CostEvaluationContext<'_>,
) -> Result<(), ReluctantReplanRefusal> {
    if context.calibrations.len() > super::MAXIMUM_REPLACEMENT_COST_HOSTS
        || context.execution_observations.len() > super::MAXIMUM_REPLAN_OBSERVATIONS
    {
        return Err(ReluctantReplanRefusal::Capacity);
    }
    let revision = forecast.source().calibration();
    if !context.calibrations.contains(&revision) {
        return Err(CostEvidenceRefusal::Scope(super::CostScopeChange::Calibration).into());
    }
    let mut current = context.execution_observations.iter().filter(|o| {
        o.source().offer() == forecast.source().offer()
            && o.source().resource() == forecast.source().resource()
    });
    let selected = current.next();
    if current.next().is_some() {
        return Err(CostEvidenceRefusal::AmbiguousEvidence.into());
    }
    forecast.validate_for_execution(context.host, revision, context.now, selected)?;
    Ok(())
}
