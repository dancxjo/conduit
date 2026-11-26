//! Explicit planning surface for a caller-retained JSON directory and grant.
use crate::{json_snapshot_file::SelectedJsonSnapshotResidence, StdHost};
use conduit_core::*;

impl SelectedJsonSnapshotResidence {
    /// Offer this one retained selection on an idle Host. This changes planning
    /// availability only: the caller must retain this selection through execution
    /// and separately provide its original authority to planning. Root must have
    /// admitted actual durable filesystem and disk capacity. Memory must come
    /// from an existing Host pool; this method creates no memory capacity.
    pub fn install_offer(&self, host: &mut StdHost) -> Result<(), String> {
        host.kernel_resources.require_reference_reconfiguration()?;
        if !host.kernel_resources.is_idle() {
            return Err("JSON snapshot selection requires an idle Host".into());
        }
        let original = &self.original;
        if original.host_id != host.advertisement.host_id
            || original.boot_id != host.advertisement.boot_id
        {
            return Err("JSON snapshot selection belongs to another Host or Boot".into());
        }
        let write = match original.host_call_contract_id.as_str() {
            conduit_std_offers::JSON_SNAPSHOT_PUBLISH_CALL => true,
            conduit_std_offers::JSON_SNAPSHOT_READ_CALL => false,
            _ => return Err("JSON snapshot selection has no matching operation".into()),
        };
        let content = ResourceContentRequirement {
            identity: self.reference.identity,
            version: self.reference.lifetime.version,
            content_profile: self.reference.content_profile.clone(),
            maximum_bytes: conduit_std_offers::JSON_SNAPSHOT_MAXIMUM_BYTES,
            maximum_items: 1,
            retention: ResourceRetention::ExternalDurable,
            sharing: ResourceSharing::SingleWriterPublished,
            access: if write {
                ResourceAccessMode::WriteCandidatePublish
            } else {
                ResourceAccessMode::ReadPublished
            },
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: u16::from(write),
            sensitive: true,
        };
        let offer = conduit_std_offers::json_snapshot_offer(content.clone())?;
        if original.capability_id != offer.capability_id
            || original.subject_kind != offer.kind_id
            || original.contract_id.as_str() != conduit_std_offers::JSON_SNAPSHOT_AUTHORITY
        {
            return Err("JSON snapshot selection does not retain the offered authority".into());
        }
        let mut advertisement = host.advertisement.clone();
        advertisement.offer_generation = OfferGeneration(
            advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or("JSON offer generation exhausted")?,
        );
        advertisement.capabilities.retain(|offered| {
            ![
                conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND,
                conduit_semantic_catalog::SNAPSHOT_READ_KIND,
            ]
            .contains(&offered.kind_id.as_str())
        });
        advertisement.capabilities.push(offer);
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        advertisement
            .resources
            .retain(|resource| resource.pool_id.as_str() != "std/selected-json-snapshot");
        advertisement.resources.push(ResourceOffer {
            pool_id: "std/selected-json-snapshot".into(),
            class_id: self.reference.access_class.clone(),
            capacity_units: 1,
            compute: None,
            content: Some(ResourceContentOffer {
                contract: content,
                owner_host: advertisement.host_id.clone(),
                owner_boot: advertisement.boot_id.clone(),
                base_id: conduit_std_offers::JSON_SNAPSHOT_BASE.into(),
                residence_profile: kind_id(conduit_std_offers::JSON_SNAPSHOT_RESIDENCE),
            }),
        });
        advertisement.resources.sort();
        let ledger = host.kernel_resources.replacement_for(&advertisement)?;
        host.advertisement = advertisement;
        host.kernel_resources = ledger;
        Ok(())
    }
}
