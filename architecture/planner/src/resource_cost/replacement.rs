//! Finite replacement-overhead facts for one unchanged logical Gear.
//! Costs do not propose, seal, install or mutate a Plan. Hysteresis and savings
//! policy consume these facts separately from service/workload forecasts.
use super::{
    CostDuration, CostDurationRange, CostEvaluationContext, CostEvidenceRefusal,
    CostEvidenceSource, CostMeasuredDuration, CostResidenceRefusal, CostResidentResource,
};
use conduit_core::{
    verify_plan, BoundedResourceRef, ExactNumericWork, ExactRational, Plan, PlannedGear,
    QuantityRole, RationalQuantity, Unit,
};

pub const MAXIMUM_REPLACEMENT_COST_RESIDENCES: usize = 64;
pub const MAXIMUM_REPLACEMENT_COST_HOSTS: usize = 16;

/// Residency is the Time cost of retaining/reestablishing residence, not bytes
/// treated as time. Actual extent, lifetime and reservations remain in the
/// separate borrowed provider/Plan evidence.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplacementCostStage {
    Preparation,
    Transfer,
    Residency,
    Migration,
    Continuity,
}
const STAGES: [ReplacementCostStage; 5] = [
    ReplacementCostStage::Preparation,
    ReplacementCostStage::Transfer,
    ReplacementCostStage::Residency,
    ReplacementCostStage::Migration,
    ReplacementCostStage::Continuity,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementCostRefusal {
    Evidence(CostEvidenceRefusal),
    Residence(CostResidenceRefusal),
    InvalidPlan,
    LogicalWorkChanged,
    MissingStage,
    MissingResourceWorkload,
    ResourceWorkloadChanged,
    DuplicateStageEvidence,
    MissingContext,
    AmbiguousContext,
    MeasurementScopeChanged,
    MissingResidence,
    DuplicateResidence,
    UnsupportedResource,
    UnsupportedTransfer,
    Capacity,
}
impl From<CostEvidenceRefusal> for ReplacementCostRefusal {
    fn from(value: CostEvidenceRefusal) -> Self {
        Self::Evidence(value)
    }
}

/// A completed, identified stage observation. Zero elapsed ticks keep the
/// clock-derived upper uncertainty; absence is never represented as zero.
/// The producer must separately establish what operation its Sign measured.
pub struct ReplacementCostComponent<'a> {
    stage: ReplacementCostStage,
    source: &'a CostEvidenceSource<'a>,
    plan: &'a Plan,
    measurement: &'a CostMeasuredDuration,
    resource_reference: Option<&'a BoundedResourceRef>,
}
impl<'a> ReplacementCostComponent<'a> {
    pub fn bind(
        stage: ReplacementCostStage,
        source: &'a CostEvidenceSource<'a>,
        plan: &'a Plan,
        measurement: &'a CostMeasuredDuration,
        resource_reference: Option<&'a BoundedResourceRef>,
    ) -> Result<Self, ReplacementCostRefusal> {
        if !verify_plan(plan) {
            return Err(ReplacementCostRefusal::InvalidPlan);
        }
        if measurement.completed() != source.window().observed()
            || measurement.completed().clock().host_id() != &source.host().host_id
            || measurement.completed().clock().boot_id() != &source.host().boot_id
            || !plan
                .fragments
                .iter()
                .flat_map(|f| &f.placements)
                .any(|p| super::residence::matches_placement_source(p, source))
        {
            return Err(ReplacementCostRefusal::MeasurementScopeChanged);
        }
        match (source.resource(), resource_reference) {
            (Some(pool), Some(reference)) => {
                let content = pool
                    .content
                    .as_ref()
                    .ok_or(ReplacementCostRefusal::UnsupportedResource)?;
                if content.contract.accepts_reference(reference).is_err()
                    || reference.access_class != pool.class_id
                {
                    return Err(ReplacementCostRefusal::ResourceWorkloadChanged);
                }
            }
            (Some(_), None) => return Err(ReplacementCostRefusal::MissingResourceWorkload),
            (None, Some(_)) => return Err(ReplacementCostRefusal::ResourceWorkloadChanged),
            (None, None) => {}
        }
        Ok(Self {
            stage,
            source,
            plan,
            measurement,
            resource_reference,
        })
    }
    pub const fn stage(&self) -> ReplacementCostStage {
        self.stage
    }
    pub fn source(&self) -> &'a CostEvidenceSource<'a> {
        self.source
    }
    pub fn measurement(&self) -> &'a CostMeasuredDuration {
        self.measurement
    }
    pub fn resource_reference(&self) -> Option<&'a BoundedResourceRef> {
        self.resource_reference
    }
    pub fn plan(&self) -> &'a Plan {
        self.plan
    }
}

/// Already-checked comparison input, with full borrowed explanation. The total
/// is a sequential overhead model; it is not a whole-program stopwatch result,
/// overlapping-stage estimate, execution guarantee or profitability decision.
pub struct ReplacementCostFacts<'a> {
    previous: &'a Plan,
    candidate: &'a Plan,
    previous_gear: &'a PlannedGear,
    candidate_gear: &'a PlannedGear,
    components: [&'a ReplacementCostComponent<'a>; 5],
    residences: &'a [&'a CostResidentResource<'a>],
    total: CostDurationRange,
}
impl<'a> ReplacementCostFacts<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        previous: &'a Plan,
        previous_gear: &'a PlannedGear,
        candidate: &'a Plan,
        candidate_gear: &'a PlannedGear,
        components: [Option<&'a ReplacementCostComponent<'a>>; 5],
        residences: &'a [&'a CostResidentResource<'a>],
        contexts: &[CostEvaluationContext<'_>],
        work: &mut ExactNumericWork,
    ) -> Result<Self, ReplacementCostRefusal> {
        use ReplacementCostRefusal as R;
        if residences.len() > MAXIMUM_REPLACEMENT_COST_RESIDENCES
            || previous_gear.resources.len() > MAXIMUM_REPLACEMENT_COST_RESIDENCES
            || candidate_gear.resources.len() > MAXIMUM_REPLACEMENT_COST_RESIDENCES
            || contexts.len() > MAXIMUM_REPLACEMENT_COST_HOSTS
        {
            return Err(R::Capacity);
        }
        if !verify_plan(previous) || !verify_plan(candidate) {
            return Err(R::InvalidPlan);
        }
        let contains = |plan: &Plan, gear: &PlannedGear| {
            plan.fragments
                .iter()
                .flat_map(|f| &f.placements)
                .any(|g| g == gear)
        };
        if !contains(previous, previous_gear)
            || !contains(candidate, candidate_gear)
            || previous.source_document_id != candidate.source_document_id
            || previous.checked_plot_id != candidate.checked_plot_id
            || previous.expanded_plot_id != candidate.expanded_plot_id
            || previous_gear.gear_id != candidate_gear.gear_id
            || previous_gear.kind_id != candidate_gear.kind_id
            || previous_gear.kind_contract_revision != candidate_gear.kind_contract_revision
            || previous_gear.inputs != candidate_gear.inputs
            || previous_gear.outputs != candidate_gear.outputs
            || previous_gear.source_span != candidate_gear.source_span
            || previous_gear.configuration != candidate_gear.configuration
            || previous_gear.semantic_contract != candidate_gear.semantic_contract
        {
            return Err(R::LogicalWorkChanged);
        }
        // This profile measures local provider transfers. A different Host or
        // Boot needs actual route/remote-owner evidence, not these local costs.
        if previous_gear.host_id != candidate_gear.host_id
            || previous_gear.boot_id != candidate_gear.boot_id
        {
            return Err(R::UnsupportedTransfer);
        }
        if components
            .iter()
            .zip(STAGES)
            .any(|(value, stage)| value.is_none_or(|c| c.stage != stage))
        {
            return Err(R::MissingStage);
        }
        let components = components.map(|c| c.expect("complete stages checked"));
        for (index, component) in components.iter().enumerate() {
            let matches = |plan: &Plan, gear: &PlannedGear| {
                component.plan == plan
                    && super::residence::matches_placement_source(gear, component.source)
            };
            if !matches(previous, previous_gear) && !matches(candidate, candidate_gear) {
                return Err(R::MeasurementScopeChanged);
            }
            if components[..index]
                .iter()
                .any(|old| old.source.sign() == component.source.sign())
            {
                return Err(R::DuplicateStageEvidence);
            }
            let gear = if component.plan == previous {
                previous_gear
            } else {
                candidate_gear
            };
            if !gear.resources.is_empty() && component.resource_reference.is_none() {
                return Err(R::MissingResourceWorkload);
            }
            validate_source(component.source, contexts)?;
        }
        for (index, residence) in residences.iter().enumerate() {
            if !((residence.plan() == previous && residence.placement() == previous_gear)
                || (residence.plan() == candidate && residence.placement() == candidate_gear))
            {
                return Err(R::MissingResidence);
            }
            if residences[..index].iter().any(|old| {
                old.plan().plan_id == residence.plan().plan_id
                    && old.placement().placement_id == residence.placement().placement_id
                    && old.provider().cost_binding() == residence.provider().cost_binding()
            }) {
                return Err(R::DuplicateResidence);
            }
            let context = context_for(residence.source(), contexts)?;
            residence
                .validate_at(context, residence.owner())
                .map_err(R::Residence)?;
        }
        for (plan, gear) in [(previous, previous_gear), (candidate, candidate_gear)] {
            for binding in &gear.resources {
                // This first concrete profile observes published content owners.
                // Scratch/compute/device residency needs its own actual observer.
                if binding.content.is_none() {
                    return Err(R::UnsupportedResource);
                }
                if !residences.iter().any(|r| {
                    r.plan() == plan
                        && r.placement() == gear
                        && r.provider().cost_binding() == binding
                }) {
                    return Err(R::MissingResidence);
                }
            }
        }
        for component in components {
            if let Some(reference) = component.resource_reference {
                let pool = component
                    .source
                    .resource()
                    .expect("bound resource workload");
                if !residences.iter().any(|r| {
                    r.plan() == component.plan
                        && r.provider().cost_binding().pool_id == pool.pool_id
                        && r.provider().cost_published_reference().ok() == Some(reference)
                }) {
                    return Err(R::ResourceWorkloadChanged);
                }
            }
        }
        let mut lower = ExactRational::ZERO;
        let mut upper = ExactRational::ZERO;
        for component in components {
            let range = component.measurement.range();
            lower = lower
                .checked_add(range.lower().seconds(), work)
                .map_err(CostEvidenceRefusal::from)?;
            upper = upper
                .checked_add(range.upper().seconds(), work)
                .map_err(CostEvidenceRefusal::from)?;
        }
        let mut duration = |value| -> Result<CostDuration, CostEvidenceRefusal> {
            CostDuration::admit(
                RationalQuantity::new(
                    value,
                    Unit::resolve("s").expect("Time Unit"),
                    QuantityRole::Linear,
                )?,
                work,
            )
        };
        let lower = duration(lower)?;
        let upper = duration(upper)?;
        let total = CostDurationRange::admit(lower, upper, work)?;
        Ok(Self {
            previous,
            candidate,
            previous_gear,
            candidate_gear,
            components,
            residences,
            total,
        })
    }
    pub fn previous(&self) -> &'a Plan {
        self.previous
    }
    pub fn candidate(&self) -> &'a Plan {
        self.candidate
    }
    pub fn gears(&self) -> (&'a PlannedGear, &'a PlannedGear) {
        (self.previous_gear, self.candidate_gear)
    }
    pub fn components(&self) -> &[&'a ReplacementCostComponent<'a>; 5] {
        &self.components
    }
    pub fn residences(&self) -> &'a [&'a CostResidentResource<'a>] {
        self.residences
    }
    pub const fn sequential_overhead(&self) -> CostDurationRange {
        self.total
    }
}

fn context_for<'a, 'b>(
    source: &CostEvidenceSource<'_>,
    contexts: &'a [CostEvaluationContext<'b>],
) -> Result<&'a CostEvaluationContext<'b>, ReplacementCostRefusal> {
    let mut matching = contexts
        .iter()
        .filter(|c| c.host.host_id == source.host().host_id);
    let context = matching
        .next()
        .ok_or(ReplacementCostRefusal::MissingContext)?;
    if matching.next().is_some() {
        return Err(ReplacementCostRefusal::AmbiguousContext);
    }
    Ok(context)
}
fn validate_source(
    source: &CostEvidenceSource<'_>,
    contexts: &[CostEvaluationContext<'_>],
) -> Result<(), ReplacementCostRefusal> {
    let context = context_for(source, contexts)?;
    if !context.calibrations.contains(&source.calibration()) {
        return Err(CostEvidenceRefusal::Scope(super::CostScopeChange::Calibration).into());
    }
    source.validate_at(context.host, source.calibration(), context.now)?;
    Ok(())
}
