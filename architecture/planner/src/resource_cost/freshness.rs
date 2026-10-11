//! Cost timestamps are exact clock-scoped facts, not shared wall-clock numbers.
use super::CostEvidenceRefusal as Refusal;
use conduit_core::MonotonicInstant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostObservationWindow {
    observed: MonotonicInstant,
    expires: MonotonicInstant,
}
impl CostObservationWindow {
    pub fn admit(observed: MonotonicInstant, expires: MonotonicInstant) -> Result<Self, Refusal> {
        expires
            .elapsed_since(&observed)
            .map_err(Refusal::InvalidClock)?;
        if expires.clock() != observed.clock() {
            return Err(Refusal::ClockQualityChanged);
        }
        if expires.ticks() == observed.ticks() {
            return Err(Refusal::InvalidWindow);
        }
        Ok(Self { observed, expires })
    }
    pub fn observed(&self) -> &MonotonicInstant {
        &self.observed
    }
    pub fn expires(&self) -> &MonotonicInstant {
        &self.expires
    }
    /// An evaluation sample needs the same validated Host, Boot, clock basis,
    /// scale and quality. Cross-Host absolute ticks cannot establish freshness.
    pub fn validate_at(&self, now: &MonotonicInstant) -> Result<(), Refusal> {
        now.validate().map_err(Refusal::InvalidClock)?;
        if now.clock() != self.observed.clock() {
            now.elapsed_since(&self.observed)
                .map_err(Refusal::InvalidClock)?;
            return Err(Refusal::ClockQualityChanged);
        }
        if now.ticks() < self.observed.ticks() {
            return Err(Refusal::FutureObservation);
        }
        if now.ticks() >= self.expires.ticks() {
            return Err(Refusal::Stale);
        }
        Ok(())
    }
}
