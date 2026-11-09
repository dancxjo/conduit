//! Receipt-free installed continuation selection. This is NOT Plan approval.
use crate::execution::Policy;
use crate::execution_authority::Refusal;
use conduit_body as b;
use conduit_core as c;

/// Borrowed actual Root facts checked against the owner-signed installed intent.
/// It cannot pretend that a prior-Boot credential is current membership. Native
/// issue additionally needs an opaque fresh canonical continuation scope.
#[derive(Clone, Copy)]
pub struct ContinuationNativeContext<'a> {
    pub(crate) bootstrap: &'a b::MemberBootstrap<'a>,
    pub(crate) actual: &'a c::HostAdvertisement,
    pub(crate) verified: &'a b::VerifiedContinuationIntent<'a>,
    pub(crate) image_id: &'a str,
    pub(crate) configuration_id: &'a str,
}
impl<'a> ContinuationNativeContext<'a> {
    pub fn check_installation(
        bootstrap: &'a b::MemberBootstrap<'a>,
        actual: &'a c::HostAdvertisement,
        verified: &'a b::VerifiedContinuationIntent<'a>,
        image_id: &'a str,
        configuration_id: &'a str,
        policy: Policy<'_>,
    ) -> Result<Self, Refusal> {
        let context = Self {
            bootstrap,
            actual,
            verified,
            image_id,
            configuration_id,
        };
        context.recheck(policy)?;
        Ok(context)
    }
    pub(crate) fn recheck(&self, policy: Policy<'_>) -> Result<(), Refusal> {
        let (Some(preparation), Some(execution)) = (policy.preparation, policy.execution) else {
            return Err(Refusal::Scope);
        };
        let Some(max) = execution.continuation else {
            return Err(Refusal::Scope);
        };
        let Some(retained) = self.bootstrap.admission.credential() else {
            return Err(Refusal::Scope);
        };
        let intent = self.verified.intent();
        let c = intent.ceilings;
        if !preparation.has_finite_bounds()
            || !max.has_finite_bounds()
            || !matches!(
                preparation.authorizer,
                b::HostPreparationAuthorizer::SeedOwner
            )
            || !matches!(execution.authorizer, b::HostExecutionAuthorizer::SeedOwner)
            || policy.active_worksets >= execution.maximum_active_worksets
            || u32::from(policy.active_worksets) >= c.active_worksets
            || c.assignment_bytes > max.assignment_bytes
            || c.runtime_bytes > max.runtime_bytes
            || c.durable_bytes > max.durable_bytes
            || c.operation_slots > max.operation_slots
            || c.active_worksets > max.active_worksets
            || c.assignment_bytes > preparation.maximum_assignment_bytes
            || c.runtime_bytes > preparation.maximum_runtime_state_bytes
            || !self.bootstrap.matches_native(
                self.actual.host_id.as_str(),
                self.image_id,
                self.configuration_id,
            )
            || self.actual.protocol_version != c::PROTOCOL_VERSION
            || self.actual.offer_generation.0 == 0
            || self.actual.boot_id.as_str() != self.verified.actual_boot()
            || self.actual.profile.as_str() != self.configuration_id
            || intent.body_id != self.bootstrap.body_id.as_str()
            || intent.host_id != self.actual.host_id.as_str()
            || intent.image_id != self.image_id
            || intent.configuration_id != self.configuration_id
            || intent.issuer_host != self.bootstrap.owner.host_id.as_str()
            || intent.issuer_key != self.bootstrap.owner.verifying_key
            || retained.body_id != self.bootstrap.body_id
            || retained.host_id != self.actual.host_id
            || retained.part_id.as_str() != intent.part_id
            || retained.credential_id.as_str() != intent.credential_id
        {
            return Err(Refusal::Scope);
        }
        Ok(())
    }
    pub fn verified_intent(&self) -> &b::VerifiedContinuationIntent<'a> {
        self.verified
    }
}

#[cfg(test)]
#[path = "continuation_native_tests.rs"]
mod tests;
