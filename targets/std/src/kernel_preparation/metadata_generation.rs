//! A checked declaration change preserves the original idle resource ledger.
use super::*;
pub(crate) struct PreparedDeclaredNameGeneration(conduit_core::OfferGeneration);
impl KernelResourceLedger {
    pub(crate) fn prepare_declared_name_generation(
        &self,
        current: &HostAdvertisement,
    ) -> Result<PreparedDeclaredNameGeneration, String> {
        if current.protocol_version != PROTOCOL_VERSION
            || self.identity
                != (
                    current.host_id.clone(),
                    current.boot_id.clone(),
                    current.offer_generation,
                )
            || !self.is_idle()
            || self.pools.len() != current.resources.len()
            || self.instances.len() != current.capabilities.len()
            || self.pools.iter().any(|p| {
                current
                    .resources
                    .iter()
                    .filter(|r| p.matches_offer(r))
                    .count()
                    != 1
            })
            || self.instances.iter().any(|(id, _)| {
                current
                    .capabilities
                    .iter()
                    .filter(|c| &c.capability_id == id)
                    .count()
                    != 1
            })
        {
            return Err("Host declaration requires exact unchanged idle resource custody".into());
        }
        Ok(PreparedDeclaredNameGeneration(
            conduit_core::OfferGeneration(
                current
                    .offer_generation
                    .0
                    .checked_add(1)
                    .ok_or("Host offer generation exhausted")?,
            ),
        ))
    }
    pub(crate) fn commit_declared_name_generation(
        &mut self,
        prepared: PreparedDeclaredNameGeneration,
    ) {
        self.identity.2 = prepared.0;
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn declared_name_generation_preserves_original_ledger_backing() {
        let mut host = crate::StdHost::new();
        let pools = host.kernel_resources.pools.as_ptr();
        let instances = host.kernel_resources.instances.as_ptr();
        let resources = host.advertisement.resources.as_ptr();
        let previous = host.advertisement.offer_generation;
        let name = conduit_core::HostDeclaredName::new("Green").unwrap();
        host.set_declared_name(name.clone()).unwrap();
        assert_eq!(host.advertisement.offer_generation.0, previous.0 + 1);
        assert_eq!(
            host.kernel_resources.identity,
            (
                host.advertisement.host_id.clone(),
                host.advertisement.boot_id.clone(),
                host.advertisement.offer_generation
            )
        );
        assert_eq!(pools, host.kernel_resources.pools.as_ptr());
        assert_eq!(instances, host.kernel_resources.instances.as_ptr());
        assert_eq!(resources, host.advertisement.resources.as_ptr());
        host.set_declared_name(name).unwrap();
        assert_eq!(host.advertisement.offer_generation.0, previous.0 + 1);
    }
    #[test]
    fn declared_name_refuses_active_or_changed_inventory_without_generation_change() {
        let mut host = crate::StdHost::new();
        let previous = host.advertisement.offer_generation;
        host.kernel_resources.instances[0].1 = 1;
        assert!(host
            .set_declared_name(conduit_core::HostDeclaredName::new("Green").unwrap())
            .is_err());
        host.kernel_resources.instances[0].1 = 0;
        host.advertisement.resources[0].capacity_units += 1;
        assert!(host
            .set_declared_name(conduit_core::HostDeclaredName::new("Green").unwrap())
            .is_err());
        assert_eq!(host.advertisement.offer_generation, previous);
        assert!(host.advertisement.declared_name.is_none());
    }
}
