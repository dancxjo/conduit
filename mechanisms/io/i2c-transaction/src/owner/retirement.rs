//! Return exact native custody only after explicit revocation and quiescence.
use super::*;
use conduit_core::{CapabilityInspection, CapabilityLifecycle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cRetirementRefusal {
    Unavailable,
    InFlight,
    NotRevoked,
}

impl<P: I2cProvider> I2cHostCall<P> {
    pub fn inspection(&self) -> &CapabilityInspection {
        self.table
            .inspections()
            .next()
            .expect("single prepared capability")
    }

    /// Return this exact provider and attachment once, retaining the owner on
    /// refusal. Issued or exhausted possession is not revoked possession.
    /// Synchronous invocation must have returned before this exclusive entrance.
    /// No capability, claim, resource release or new execution authority survives
    /// the transfer. Revocation may quarantine the physical provider: returning
    /// custody does not reset it or assert that it is Ready for another Play.
    pub fn retire_retained(
        owner: &mut Option<Self>,
    ) -> Result<(P, I2cAttachment), I2cRetirementRefusal> {
        let current = owner.as_ref().ok_or(I2cRetirementRefusal::Unavailable)?;
        let inspection = current.inspection();
        if inspection.in_flight_operations != 0 {
            return Err(I2cRetirementRefusal::InFlight);
        }
        if inspection.lifecycle != CapabilityLifecycle::Revoked {
            return Err(I2cRetirementRefusal::NotRevoked);
        }
        let current = owner.take().expect("preflighted retained owner");
        Ok((current.provider, current.attachment))
    }
}
