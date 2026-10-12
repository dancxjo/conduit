//! Root-initialized storage resources; class wire meaning remains ordinary Source.
use super::*;
use crate::storage_base::{
    transaction_contract::BlockReadTransactionContract, transaction_factory::*,
};

pub(crate) struct NativeStoragePreparation {
    pub host: HostId,
    pub boot: BootId,
    pub provider: BaseInstanceId,
    pub pool: ResourcePoolId,
    pub artifact: ArtifactId,
    pub maximum_retained_bytes: u64,
    pub authorize: NativeProtocolIssuer,
    pub settle: NativeProtocolIssuer,
}
impl NativeStoragePreparation {
    pub(super) fn append(
        &self,
        advertisement: &mut HostAdvertisement,
    ) -> Result<(), crate::protocol_host_calls::ProtocolCallRefusal> {
        use crate::protocol_host_calls::ProtocolCallRefusal as E;
        if advertisement.host_id != self.host
            || advertisement.boot_id != self.boot
            || advertisement
                .bases
                .iter()
                .any(|base| base.provider_instance_id == self.provider)
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.pool)
            || advertisement.capabilities.iter().any(|capability| {
                matches!(
                    capability.capability_id.as_str(),
                    AUTHORIZE_IMPLEMENTATION | SETTLE_IMPLEMENTATION
                )
            })
        {
            return Err(E::InvalidPlan);
        }
        let contract = BlockReadTransactionContract::prepare().map_err(|_| E::InvalidPlan)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 2,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(advertisement.capabilities.len() + 2)
                .map_err(|_| E::InvalidPlan)?,
            maximum_advertised_resources: u16::try_from(advertisement.resources.len() + 1)
                .map_err(|_| E::InvalidPlan)?,
        })
        .map_err(|_| E::InvalidPlan)?;
        registry
            .register(BaseProviderEntry {
                base_id: self.provider.as_str().into(),
                provider_instance_id: self.provider.clone(),
                provider_generation: 1,
                implementation_id: TRANSACTION_BASE.into(),
                mechanism_family: TRANSACTION_RESOURCE.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![
                    offer(
                        StorageTransactionRole::Authorize,
                        &contract,
                        self.artifact.clone(),
                    ),
                    offer(
                        StorageTransactionRole::Settle,
                        &contract,
                        self.artifact.clone(),
                    ),
                ],
                // Two finite roles share the independently backed association envelope.
                resources: vec![resource_offer(self.pool.as_str(), TRANSACTION_RESOURCE, 2)],
            })
            .map_err(|_| E::InvalidPlan)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| E::InvalidPlan)
    }
}
