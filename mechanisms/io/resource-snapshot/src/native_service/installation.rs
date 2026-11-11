use super::*;
pub struct ReadyRecordService<S> {
    pub(super) identity: AppendNativeIdentity,
    pub(super) provider: PreparedNorRecordResidence<S>,
    pub(super) originals: [c::AuthorityGrant; 2],
    pub(super) limits: ServiceLimits,
    pub(super) append: crate::native_append::result::Encoder,
    pub(super) read: crate::read_result::Encoder,
}
impl<S: NorStorage> ReadyRecordService<S> {
    /// Retains actual recovered ownership and independently original grants.
    /// No media IO. Every refusal returns all affine inputs unchanged.
    /// # Safety
    /// Root owns/funds the exact residence/generation, original current-Boot
    /// grants, all inline/heap buffers, finite operation budgets, and lawful
    /// synchronous media windows excluding timing-sensitive IO. Recovery gaps
    /// remain observable. Only genuine kernel acceptance may acknowledge a result.
    #[allow(clippy::type_complexity, clippy::result_large_err)]
    pub unsafe fn new_retained(
        identity: AppendNativeIdentity,
        provider: PreparedNorRecordResidence<S>,
        originals: [c::AuthorityGrant; 2],
        limits: ServiceLimits,
    ) -> Result<
        Self,
        (
            AppendNativeIdentity,
            PreparedNorRecordResidence<S>,
            [c::AuthorityGrant; 2],
            ServiceRefusal,
        ),
    > {
        let check = || {
            if identity.provider_generation == 0
                || [
                    &identity.host_id.0,
                    &identity.boot_id.0,
                    &identity.base_id.0,
                    &identity.provider_instance_id.0,
                    &identity.resource_pool_id.0,
                    &identity.resource_generation_id.0,
                    &identity.envelope_id.0,
                    &identity.artifact_id.0,
                    &identity.implementation_id.0,
                    &identity.base_implementation_id.0,
                ]
                .iter()
                .any(|v| v.is_empty())
                || limits.append_operations == 0
                || limits.read_operations == 0
            {
                return Err(ServiceRefusal::WrongBinding);
            }
            if !provider.recovered() {
                return Err(ServiceRefusal::Media(
                    RecordResidenceRefusal::RecoveryRequired,
                ));
            }
            for (i, grant) in originals.iter().enumerate() {
                let (contract, call, kind) = if i == 0 {
                    (
                        crate::native_append::APPEND_AUTHORITY,
                        crate::native_append::APPEND_OPERATION,
                        crate::append_contract::APPEND_KIND,
                    )
                } else {
                    (
                        READ_AUTHORITY,
                        READ_OPERATION,
                        crate::read_contract::READ_KIND,
                    )
                };
                if grant.grant_id.as_str().is_empty()
                    || grant.host_id != identity.host_id
                    || grant.boot_id != identity.boot_id
                    || grant.capability_id.as_str() != SERVICE_CAPABILITY
                    || grant.contract_id.as_str() != contract
                    || grant.host_call_contract_id.as_str() != call
                    || grant.subject_kind.as_str() != kind
                {
                    return Err(ServiceRefusal::WrongBinding);
                }
            }
            if originals[0].grant_id == originals[1].grant_id {
                return Err(ServiceRefusal::WrongBinding);
            }
            Ok(())
        };
        if let Err(error) = check() {
            return Err((identity, provider, originals, error));
        }
        let append = match crate::native_append::result::Encoder::new() {
            Ok(v) => v,
            Err(_) => return Err((identity, provider, originals, ServiceRefusal::Possession)),
        };
        let read = match crate::read_result::Encoder::new() {
            Ok(v) => v,
            Err(_) => return Err((identity, provider, originals, ServiceRefusal::Possession)),
        };
        Ok(Self {
            identity,
            provider,
            originals,
            limits,
            append,
            read,
        })
    }
    pub fn native_identity(&self) -> &AppendNativeIdentity {
        &self.identity
    }
    pub fn original_grants(&self) -> &[c::AuthorityGrant; 2] {
        &self.originals
    }
    pub fn limits(&self) -> ServiceLimits {
        self.limits
    }
    pub fn residence_id(&self) -> RecordResidenceId {
        self.provider.residence_id()
    }
    pub fn remaining_slots(&self) -> Option<u32> {
        self.provider.remaining_slots()
    }
    /// Prepared codec/identity/grant heap payload. Inline state, media provider
    /// backing and allocator overhead remain separately funded.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut total = self.append.backing()?.checked_add(self.read.backing()?)?;
        for text in [
            &self.identity.host_id.0,
            &self.identity.boot_id.0,
            &self.identity.base_id.0,
            &self.identity.provider_instance_id.0,
            &self.identity.resource_pool_id.0,
            &self.identity.resource_generation_id.0,
            &self.identity.envelope_id.0,
            &self.identity.artifact_id.0,
            &self.identity.implementation_id.0,
            &self.identity.base_implementation_id.0,
        ] {
            total = total.checked_add(text.capacity())?;
        }
        for grant in &self.originals {
            for text in [
                &grant.grant_id.0,
                &grant.contract_id.0,
                &grant.host_call_contract_id.0,
                &grant.subject_kind.0,
                &grant.host_id.0,
                &grant.boot_id.0,
                &grant.capability_id.0,
            ] {
                total = total.checked_add(text.capacity())?;
            }
        }
        Some(total)
    }
}
