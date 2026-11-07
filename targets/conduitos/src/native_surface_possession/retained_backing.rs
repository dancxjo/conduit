//! Charge actual retained possession, not a descriptive or unused issuer grant.
use super::*;
impl ActiveNativeSurfacePossession {
    /// Actual owned heap payload only. Inline Self and allocator overhead are
    /// charged by the enclosing owner. Optional BaseOperationLease is entirely
    /// inline (bearer bytes and scalar generations), so adds no heap payload.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.table
            .retained_backing_bytes()?
            .checked_add(self.cord.retained_backing_bytes()?)?
            .checked_add(self.claim.retained_backing_bytes()?)
    }
}
impl NativeSurfacePossessionReceipt {
    /// The lifecycle is static and the generation inline; only these three IDs
    /// retain owned String allocations. No serialized receipt is allocated here.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.connection_id
            .0
            .capacity()
            .checked_add(self.resource_pool_id.0.capacity())?
            .checked_add(self.provider_instance_id.0.capacity())
    }
}

#[cfg(all(test, feature = "native-compositor-core"))]
mod tests {
    use super::*;
    #[test]
    fn actual_resource_cord_owner_is_charged_before_authorize_and_retired_after_use() {
        // CPU possession lifecycle fixture; no physical output/ACK is asserted.
        let provider = crate::product_bases::fixture_surface_provider();
        let offer = conduit_conduitos_mask_offer::native_read_only_host_advertisement(
            &"host/possession".into(),
            &"boot/possession".into(),
            1,
            &provider.entry,
            conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
        );
        let stage = conduit_conduitos_mask_offer::prepare_stage_from_offer(
            conduit_conduitos_mask_offer::Adapter::Native,
            &offer,
        )
        .unwrap();
        let mut active = ActiveNativeSurfacePossession::begin(
            &provider,
            &stage.planned_mask.plan,
            "play/possession".into(),
        )
        .unwrap();
        assert_eq!(active.table.inspections().count(), 1);
        let before = active.retained_backing_bytes().unwrap();
        let prior = active.claim.plan_id.0.capacity();
        active.claim.plan_id.0.reserve(173);
        let expected = before + active.claim.plan_id.0.capacity() - prior;
        assert_eq!(active.retained_backing_bytes(), Some(expected));
        active.authorize().unwrap();
        assert!(active.lease.is_some());
        // The actual lease is inline and the table preowns active sequence slots.
        assert_eq!(active.retained_backing_bytes(), Some(expected));
        let receipt = active.complete_and_retire().unwrap();
        assert_eq!(receipt.lifecycle, "revoked-at-play-end");
        assert_eq!(
            receipt.retained_backing_bytes(),
            Some(
                receipt.connection_id.0.capacity()
                    + receipt.resource_pool_id.0.capacity()
                    + receipt.provider_instance_id.0.capacity()
            )
        );
    }
}

#[cfg(all(test, feature = "native-compositor-core"))]
mod refusal_tests {
    use super::*;
    #[test]
    fn possession_preserves_resource_and_scope_refusal_details() {
        let mut provider = crate::product_bases::fixture_surface_provider();
        let stage = conduit_conduitos_mask_offer::prepare_stage(
            conduit_conduitos_mask_offer::Adapter::Native,
            &"host/typed-refusal".into(),
            &"boot/typed-refusal".into(),
            1,
            Some(&provider.entry),
        )
        .unwrap();
        provider.entry.provider_generation += 1;
        assert!(matches!(
            ActiveNativeSurfacePossession::begin(
                &provider,
                &stage.planned_mask.plan,
                "play/typed-refusal".into()
            ),
            Err(ResourceCordRefusal::Capability(
                BaseCapabilityRefusal::WrongScope
            ))
        ));
        let mut no_resource = stage.planned_mask.plan;
        for connection in &mut no_resource.fragments[0].connections {
            connection.resource = None;
        }
        assert!(matches!(
            ActiveNativeSurfacePossession::begin(
                &provider,
                &no_resource,
                "play/typed-refusal".into()
            ),
            Err(ResourceCordRefusal::NotResourceCord)
        ));
    }
}
