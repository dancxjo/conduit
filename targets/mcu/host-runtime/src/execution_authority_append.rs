//! Original physical residence facts before the original grant exists.
use super::*;
use conduit_resource_snapshot::{
    append::PreparedNorRecordResidence,
    native_append::{AppendNativeIdentity, ReadyRecordAppender, APPEND_CAPABILITY},
    nor::NorStorage,
};
impl<'a> NativeFacts<'a> {
    /// Borrow initialized physical custody before original grant authorization.
    /// This performs no IO/allocation and grants no permission from recovery.
    /// # Safety
    /// Root exclusively owns this exact recovered residence paired with this
    /// independently initialized current identity/generation. Neither may come
    /// from an incoming assignment or a copied readiness descriptor. Root must
    /// consume THESE SAME original owners into ReadyRecordAppender after the
    /// existing Issuer authorizes its original grant, and separately fund/admit
    /// the full finite provider, preparation/work buffers and physical windows.
    pub unsafe fn recovered_append<S: NorStorage>(
        identity: &'a AppendNativeIdentity,
        residence: &'a PreparedNorRecordResidence<S>,
    ) -> Result<Self, Refusal> {
        if !residence.recovered()
            || residence.total_slots() == 0
            || residence.residence_bytes() == 0
        {
            return Err(Refusal::Scope);
        }
        if identity.provider_generation == 0
            || [
                identity.host_id.as_str(),
                identity.boot_id.as_str(),
                identity.base_id.as_str(),
                identity.provider_instance_id.as_str(),
                identity.resource_pool_id.as_str(),
                &identity.resource_generation_id.0,
                identity.envelope_id.as_str(),
                identity.artifact_id.as_str(),
                identity.implementation_id.as_str(),
                identity.base_implementation_id.as_str(),
            ]
            .iter()
            .any(|v| v.is_empty())
        {
            return Err(Refusal::Scope);
        }
        Ok(facts!(identity, APPEND_CAPABILITY))
    }
    /// Subsequent ordinary/continuation issuance borrows the actual Ready owner.
    /// Identity/fingerprint is unchanged from the recovered pre-grant borrow;
    /// Ready still checks equality against the original Issuer grant.
    pub fn append<S: NorStorage>(owner: &'a ReadyRecordAppender<S>) -> Self {
        facts!(owner.native_identity(), APPEND_CAPABILITY)
    }
}
#[cfg(test)]
#[path = "execution_authority_append_tests.rs"]
mod tests;
