//! Same recovered physical service facts and bounded original descriptions.
use super::*;
impl<'a> NativeFacts<'a> {
    pub fn record_service<S: conduit_resource_snapshot::nor::NorStorage>(
        owner: &'a conduit_resource_snapshot::native_service::ReadyRecordService<S>,
    ) -> Self {
        facts!(
            owner.native_identity(),
            conduit_resource_snapshot::native_service::SERVICE_CAPABILITY
        )
    }
    /// # Safety
    /// Root owns this exact recovered residence/identity pair, funds the finite
    /// buffers/windows, and will install THESE SAME owners into ReadyRecordService.
    /// Neither metadata nor recovery creates original operation authority.
    pub unsafe fn recovered_record_service<S: conduit_resource_snapshot::nor::NorStorage>(
        identity: &'a conduit_resource_snapshot::native_append::AppendNativeIdentity,
        residence: &'a conduit_resource_snapshot::append::PreparedNorRecordResidence<S>,
    ) -> Result<Self, Refusal> {
        // SAFETY: identical physical custody/identity checks; only the explicit
        // service capability description differs, and no grant is produced.
        unsafe { Self::recovered_append(identity, residence) }?;
        Ok(facts!(
            identity,
            conduit_resource_snapshot::native_service::SERVICE_CAPABILITY
        ))
    }
}

impl Issuer {
    /// Copy only this Issuer's exact retained original description. Cloning a
    /// description creates no additional original grant or issued capability.
    pub(crate) fn copy_original_grant(
        &self,
        remaining: &mut usize,
    ) -> Result<c::AuthorityGrant, Refusal> {
        let original = self.grant().ok_or(Refusal::Scope)?;
        let mut budget = Budget(*remaining);
        let copied = c::AuthorityGrant {
            grant_id: budget.copy(original.grant_id.as_str())?.into(),
            contract_id: budget.copy(original.contract_id.as_str())?.into(),
            host_call_contract_id: budget.copy(original.host_call_contract_id.as_str())?.into(),
            subject_kind: budget.copy(original.subject_kind.as_str())?.into(),
            host_id: budget.copy(original.host_id.as_str())?.into(),
            boot_id: budget.copy(original.boot_id.as_str())?.into(),
            capability_id: budget.copy(original.capability_id.as_str())?.into(),
        };
        *remaining = budget.0;
        Ok(copied)
    }
}
