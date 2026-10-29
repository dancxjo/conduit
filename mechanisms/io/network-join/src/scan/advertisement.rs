//! Ordinary availability projection from the retained initialized owner.
use super::*;
use alloc::vec;
use conduit_core::*;
impl<P: NetworkScanProvider> ReadyNetworkScanBase<P> {
    /// Preparation only. This neither scans nor grants native possession.
    pub fn append_to_advertisement(
        &self,
        ad: &mut HostAdvertisement,
    ) -> Result<(), ScanCallRefusal> {
        let i = self.native_identity();
        if ad.host_id != i.host
            || ad.boot_id != i.boot
            || ad
                .bases
                .iter()
                .any(|b| b.base_id == i.base || b.provider_instance_id == i.instance)
            || ad
                .capabilities
                .iter()
                .any(|c| c.capability_id.as_str() == SCAN_CAPABILITY)
            || ad.resources.iter().any(|r| r.pool_id == i.resource)
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(ad.capabilities.len().saturating_add(1))
                .map_err(|_| ScanCallRefusal::WrongBinding)?,
            maximum_advertised_resources: u16::try_from(ad.resources.len().saturating_add(1))
                .map_err(|_| ScanCallRefusal::WrongBinding)?,
        })
        .map_err(|_| ScanCallRefusal::WrongBinding)?;
        registry
            .register(BaseProviderEntry {
                base_id: i.base.clone(),
                provider_instance_id: i.instance.clone(),
                provider_generation: i.provider_generation,
                implementation_id: i.base_implementation.clone(),
                mechanism_family: SCAN_BASE_KIND.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![network_scan_offer(
                    SCAN_CAPABILITY.into(),
                    i.implementation.clone(),
                    i.artifact.clone(),
                )],
                resources: vec![resource_offer(
                    i.resource.as_str(),
                    WIFI_STATION_RESOURCE_CLASS,
                    1,
                )],
            })
            .map_err(|_| ScanCallRefusal::WrongBinding)?;
        registry
            .project_ready_into(ad)
            .map_err(|_| ScanCallRefusal::WrongBinding)
    }
}
