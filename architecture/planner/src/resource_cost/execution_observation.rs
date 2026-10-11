//! Current producer-admitted execution state, separate from forecast assumptions.
//! Neither residency nor this sample grants hard capacity or authority.
use super::{CostEvidenceRefusal as Refusal, CostEvidenceSource, CostExecutionCondition};

pub struct CostExecutionObservation<'a> {
    source: &'a CostEvidenceSource<'a>,
    condition: CostExecutionCondition,
}
impl<'a> CostExecutionObservation<'a> {
    pub fn admit(
        source: &'a CostEvidenceSource<'a>,
        condition: CostExecutionCondition,
    ) -> Result<Self, Refusal> {
        if matches!(
            condition,
            CostExecutionCondition::Warm {
                residency_revision: 0
            }
        ) {
            return Err(Refusal::InvalidIdentity);
        }
        Ok(Self { source, condition })
    }
    pub fn source(&self) -> &'a CostEvidenceSource<'a> {
        self.source
    }
    pub const fn condition(&self) -> CostExecutionCondition {
        self.condition
    }
    pub(super) fn matches_subject(&self, forecast: &CostEvidenceSource<'_>) -> Result<(), Refusal> {
        if self.source.host() != forecast.host() {
            return Err(Refusal::UnadvertisedSubject);
        }
        if self.source.offer() != forecast.offer() {
            return Err(Refusal::Scope(super::CostScopeChange::Offer));
        }
        if self.source.resource() != forecast.resource() {
            return Err(Refusal::Scope(super::CostScopeChange::Resource));
        }
        Ok(())
    }
}

impl core::fmt::Debug for CostExecutionObservation<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CostExecutionObservation")
            .field("host", &self.source.host().host_id)
            .field("boot", &self.source.host().boot_id)
            .field("capability", &self.source.offer().capability_id)
            .field("sign", &self.source.sign())
            .field("condition", &self.condition)
            .finish()
    }
}
