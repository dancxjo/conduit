//! Finite owner-derived workload measures and typed service-time prediction.
//! Metric definitions come from a reviewed catalogue, never request labels.
use super::{
    CostDemand, CostDuration, CostDurationRange, CostEvidenceRefusal as Refusal,
    CostEvidenceSource, CostForecastStages, CostWorkClass, CostedForecast, FulfillmentCostForecast,
    MAXIMUM_COST_ID_BYTES,
};
use conduit_core::{ArtifactId, ExactNumericWork, ExactRational, RationalQuantity};

pub struct CostWorkMetric<'a> {
    identity: &'a ArtifactId,
    revision: u64,
    maximum_units: u64,
    measure: fn(CostDemand<'_>) -> Result<u64, Refusal>,
}
impl<'a> CostWorkMetric<'a> {
    /// The owner supplies bounded derivation from checked meaning and admitted
    /// canonical inputs. This callback runs during preparation, outside Play.
    pub fn admit(
        identity: &'a ArtifactId,
        revision: u64,
        maximum_units: u64,
        measure: fn(CostDemand<'_>) -> Result<u64, Refusal>,
    ) -> Result<Self, Refusal> {
        if identity.as_str().is_empty()
            || identity.as_str().len() > MAXIMUM_COST_ID_BYTES
            || revision == 0
        {
            return Err(Refusal::InvalidIdentity);
        }
        if maximum_units == 0 {
            return Err(Refusal::WorkLimit);
        }
        Ok(Self {
            identity,
            revision,
            maximum_units,
            measure,
        })
    }
    pub(super) fn measure(&self, demand: CostDemand<'_>) -> Result<CostWorkEstimate<'a>, Refusal> {
        let units = (self.measure)(demand)?;
        if units > self.maximum_units {
            return Err(Refusal::WorkLimit);
        }
        Ok(CostWorkEstimate {
            metric_identity: self.identity,
            metric_revision: self.revision,
            maximum_units: self.maximum_units,
            units,
        })
    }
    fn matches(&self, estimate: CostWorkEstimate<'_>) -> bool {
        self.identity == estimate.metric_identity
            && self.revision == estimate.metric_revision
            && self.maximum_units == estimate.maximum_units
    }
}

/// Semantic units are named by the owner metric, not inferred from byte size.
/// A page, model input byte, token and compute element remain distinct measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostWorkEstimate<'a> {
    metric_identity: &'a ArtifactId,
    metric_revision: u64,
    maximum_units: u64,
    units: u64,
}
impl<'a> CostWorkEstimate<'a> {
    pub(super) fn validate_match(self, other: Self) -> Result<(), Refusal> {
        if self.metric_identity != other.metric_identity
            || self.metric_revision != other.metric_revision
            || self.maximum_units != other.maximum_units
        {
            return Err(Refusal::WorkMetricChanged);
        }
        if self.units != other.units {
            return Err(Refusal::WorkExtentChanged);
        }
        Ok(())
    }

    pub fn metric_identity(self) -> &'a ArtifactId {
        self.metric_identity
    }
    pub const fn metric_revision(self) -> u64 {
        self.metric_revision
    }
    pub const fn maximum_units(self) -> u64 {
        self.maximum_units
    }
    pub const fn units(self) -> u64 {
        self.units
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CostNonServiceStages {
    pub setup: Option<CostDurationRange>,
    pub queue: Option<CostDurationRange>,
    pub transfer: Option<CostDurationRange>,
}
/// An admitted model estimates service Time per owner-defined work unit.
/// The generic policy still compares total fulfillment Time, never raw rates.
/// This linear model is an estimate, not a future-outcome bound or a deadline.
pub struct CostServiceModel<'a> {
    source: &'a CostEvidenceSource<'a>,
    class: CostWorkClass<'a>,
    metric: &'a CostWorkMetric<'a>,
    per_work_unit: CostDurationRange,
    other: CostNonServiceStages,
}
impl<'a> CostServiceModel<'a> {
    pub fn admit(
        source: &'a CostEvidenceSource<'a>,
        class: CostWorkClass<'a>,
        metric: &'a CostWorkMetric<'a>,
        per_work_unit: CostDurationRange,
        other: CostNonServiceStages,
    ) -> Result<Self, Refusal> {
        class.validate_source(source)?;
        if other.setup.is_none() || other.queue.is_none() || other.transfer.is_none() {
            return Err(Refusal::MissingForecastStage);
        }
        Ok(Self {
            source,
            class,
            metric,
            per_work_unit,
            other,
        })
    }
    pub fn predict(
        &self,
        demand: CostDemand<'a>,
        work: &mut ExactNumericWork,
    ) -> Result<CostPreparedForecast<'a>, Refusal> {
        self.class.validate_demand(demand.class().gear())?;
        let estimate = demand.work_estimate().ok_or(Refusal::MissingWorkEstimate)?;
        if !self.metric.matches(estimate) {
            return Err(Refusal::WorkMetricChanged);
        }
        let multiplier = ExactRational::from_i128(i128::from(estimate.units));
        let scale = |duration: CostDuration,
                     work: &mut ExactNumericWork|
         -> Result<CostDuration, Refusal> {
            let original = duration.original();
            let coordinate = original.coordinate().checked_multiply(multiplier, work)?;
            CostDuration::admit(
                RationalQuantity::new(coordinate, original.unit(), original.role())?,
                work,
            )
        };
        let lower = scale(self.per_work_unit.lower(), work)?;
        let upper = scale(self.per_work_unit.upper(), work)?;
        let service = CostDurationRange::admit(lower, upper, work)?;
        let fulfillment = FulfillmentCostForecast::sequential(
            CostForecastStages {
                setup: self.other.setup,
                queue: self.other.queue,
                service: Some(service),
                transfer: self.other.transfer,
            },
            work,
        )?;
        Ok(CostPreparedForecast {
            source: self.source,
            demand,
            fulfillment,
            per_work_unit: self.per_work_unit,
        })
    }
}
/// Keeps the actual demand, metric measure, per-unit Time and producing source
/// together until binding. Owned numerical storage remains outside Play.
pub struct CostPreparedForecast<'a> {
    source: &'a CostEvidenceSource<'a>,
    demand: CostDemand<'a>,
    fulfillment: FulfillmentCostForecast,
    per_work_unit: CostDurationRange,
}
impl CostPreparedForecast<'_> {
    pub fn bind(&self) -> Result<CostedForecast<'_>, Refusal> {
        Ok(
            CostedForecast::bind(self.source, self.demand, &self.fulfillment)?
                .with_service_basis(self.per_work_unit),
        )
    }
    pub fn fulfillment(&self) -> &FulfillmentCostForecast {
        &self.fulfillment
    }
}
