//! Explicit installation of one retained Crèche encounter resource.
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_body_make::deployment_runtime_catalog::{creche_std_offer, CRECHE_RESOURCE};
use conduit_core::ResourceOffer;

impl StdHost {
    /// Construct a Host image containing Crèche support and one explicitly
    /// supplied encounter slot. This grants no source, toolchain or device
    /// authority. Execution additionally requires its scoped admitted owner.
    pub fn new_with_creche_encounter(
        config: StdHostConfig,
        composition: StdHostComposition,
        resource: ResourceOffer,
    ) -> Result<Self, String> {
        if resource.class_id.as_str() != CRECHE_RESOURCE
            || resource.pool_id.as_str().trim().is_empty()
            || resource.capacity_units != 1
            || resource.compute.is_some()
            || resource.content.is_some()
        {
            return Err("Creche installation requires one explicit encounter slot".into());
        }
        let mut advertisement =
            crate::composition::build_advertisement(config, composition, None, None, None, false);
        if advertisement
            .resources
            .iter()
            .any(|r| r.pool_id == resource.pool_id)
        {
            return Err("Creche encounter pool collides with an installed resource".into());
        }
        advertisement.resources.push(resource);
        advertisement.capabilities.push(creche_std_offer());
        advertisement.resources.sort();
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        Self::from_advertisement(advertisement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> StdHostConfig {
        StdHostConfig {
            host_id: "test/creche-install".into(),
            boot_id: "test/creche-boot".into(),
            offer_generation: conduit_core::OfferGeneration(7),
        }
    }
    #[test]
    fn creche_installation_requires_exact_explicit_single_slot() {
        let slot = conduit_core::resource_offer("test/creche-slot", CRECHE_RESOURCE, 1);
        let host = StdHost::new_with_creche_encounter(
            config(),
            StdHostComposition::default(),
            slot.clone(),
        )
        .unwrap();
        assert_eq!(host.advertisement().host_id, config().host_id);
        assert!(host.advertisement().resources.contains(&slot));
        assert!(host
            .advertisement()
            .capabilities
            .contains(&creche_std_offer()));
        for capacity in [0, 2] {
            let mut invalid = slot.clone();
            invalid.capacity_units = capacity;
            assert!(StdHost::new_with_creche_encounter(
                config(),
                StdHostComposition::default(),
                invalid
            )
            .is_err());
        }
        let mut wrong = slot;
        wrong.class_id = "test/foreign-slot".into();
        assert!(
            StdHost::new_with_creche_encounter(config(), StdHostComposition::default(), wrong)
                .is_err()
        );
    }
}
