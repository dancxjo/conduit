//! Read-only current owner/grant correspondence for canonical local construction.
//! These facts never issue a table, approve a fragment or transfer a provider.
use super::*;
pub struct ContinuationNativeDescription<'a> {
    pub binding: c::AssignedNativeBinding,
    pub instance: &'a str,
    pub grant: &'a str,
    pub pool: &'a str,
}
impl Issuer {
    #[allow(clippy::too_many_arguments)]
    pub fn describe_continuation<'a>(
        &'a self,
        context: crate::continuation_native::ContinuationNativeContext<'_>,
        policy: crate::execution::Policy<'_>,
        native: &NativeFacts<'a>,
        operation: &Operation<'_>,
        mut binding: c::AssignedNativeBinding,
    ) -> Result<ContinuationNativeDescription<'a>, Refusal> {
        context.recheck(policy)?;
        if native.host != &context.actual.host_id
            || native.boot != &context.actual.boot_id
            || native.artifact.as_str() != context.image_id
        {
            return Err(Refusal::Scope);
        }
        let (grant, fingerprint) = self.original.as_ref().ok_or(Refusal::Scope)?;
        if *fingerprint
            != fingerprint_parts(
                context.image_id,
                context.configuration_id,
                context.verified.intent().body_id,
                native,
                operation,
            )
        {
            return Err(Refusal::Scope);
        }
        let id = c::AssignedIdentity::from_text;
        let mut bases = context.actual.bases.iter().filter(|b| {
            b.provider_instance_id == *native.instance && b.provider_generation == native.generation
        });
        let base = bases.next().ok_or(Refusal::Scope)?;
        let mut pools = context
            .actual
            .resources
            .iter()
            .filter(|p| p.pool_id == *native.pool);
        let pool = pools.next().ok_or(Refusal::Scope)?;
        if bases.next().is_some()
            || pools.next().is_some()
            || base.lifecycle != c::BaseLifecycle::Ready
            || base.enforcement_class != c::BaseEnforcementClass::Cooperative
            || !base
                .capability_ids
                .iter()
                .any(|c| c.as_str() == native.capability)
            || !base.resource_pool_ids.contains(&pool.pool_id)
            || binding.base != id(base.base_id.as_str())
            || binding.base_implementation != id(base.implementation_id.as_str())
            || binding.mechanism != id(base.mechanism_family.as_str())
            || binding.capability != id(native.capability)
            || binding.enforcement != 0
            || binding.units == 0
            || binding.units > pool.capacity_units
            || binding.resource_class != id(pool.class_id.as_str())
            || pool.compute.is_some()
            || pool.content.is_some()
            || binding.authority != id(operation.authority)
            || binding.call != id(operation.call)
            || binding.subject != id(operation.kind)
            || binding.authority_capability != id(native.capability)
        {
            return Err(Refusal::Scope);
        }
        binding.instance = id(native.instance.as_str());
        binding.generation = native.generation;
        binding.grant = id(grant.grant_id.as_str());
        binding.host = id(native.host.as_str());
        binding.boot = id(native.boot.as_str());
        binding.pool = id(native.pool.as_str());
        Ok(ContinuationNativeDescription {
            binding,
            instance: native.instance.as_str(),
            grant: grant.grant_id.as_str(),
            pool: native.pool.as_str(),
        })
    }
}
