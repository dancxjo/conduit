//! Membership advertises installed local, DOM Mask, and live webchat Backs.
use conduit_core::{BootId, HostAdvertisement, HostId};

pub(crate) fn advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    let local = super::advertisement(host_id.clone(), boot_id.clone());
    let mut advertised = crate::webchat::admission_advertisement(host_id, boot_id);
    for offer in local.capabilities {
        advertised
            .capabilities
            .retain(|prior| prior.capability_id != offer.capability_id);
        advertised.capabilities.push(offer);
    }
    // The DOM Mask has its own bounded presentation executor and is not a
    // general workload Back in the ordinary browser plot runner.
    let mask = super::dom_mask::offer();
    advertised
        .capabilities
        .retain(|prior| prior.capability_id != mask.capability_id);
    advertised.capabilities.push(mask);
    for resource in local.resources {
        advertised
            .resources
            .retain(|prior| prior.pool_id != resource.pool_id);
        advertised.resources.push(resource);
    }
    advertised
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertised
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    advertised
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_preserves_every_installed_offer_within_admission_bounds() {
        let host = HostId::from("browser/member".to_owned() + &"x".repeat(96));
        let boot = BootId::from("browser/boot".to_owned() + &"x".repeat(96));
        let advertised = advertisement(host.clone(), boot.clone());
        let local = crate::installed_browser::advertisement(host, boot);
        let execution = crate::installed_browser::execution_capability_ids();
        assert_eq!(execution.len(), local.capabilities.len());
        for offer in local.capabilities {
            assert!(advertised.capabilities.contains(&offer));
            assert!(execution.contains(&offer.capability_id));
        }
        assert!(advertised
            .capabilities
            .windows(2)
            .all(|pair| pair[0].capability_id < pair[1].capability_id));
        assert!(advertised
            .resources
            .windows(2)
            .all(|pair| pair[0].pool_id < pair[1].pool_id));
        let bytes = serde_json::to_vec(&advertised).unwrap();
        assert!(
            advertised.capabilities.len() <= conduit_body::MAX_CANDIDATE_CAPABILITIES,
            "installed membership advertisement has {} capabilities, above the admitted maximum {}",
            advertised.capabilities.len(),
            conduit_body::MAX_CANDIDATE_CAPABILITIES
        );
        assert!(
            bytes.len() + 1024 <= conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize,
            "installed membership advertisement: {} bytes, {} capabilities",
            bytes.len(),
            advertised.capabilities.len()
        );
        let body = conduit_body::Body::born(
            conduit_core::SourceDocumentId::from("source/membership-budget"),
            conduit_core::CheckedPlotId::from("checked/membership-budget"),
            1,
            conduit_core::SignId::from("sign/membership-budget"),
        )
        .unwrap();
        let mut inventory = conduit_body::CandidateInventory::new(body.body_id).unwrap();
        let observation = conduit_body::CandidateObservation {
            advertisement: advertised.clone(),
            friendly_label: "Installed browser".into(),
            observed_binding_id: conduit_core::LinkBindingId::from("line/membership-budget"),
            observation_sign_id: conduit_core::SignId::from("sign/membership-observed"),
            proof_id: conduit_body::DiscoveryProofId::bind("proof/membership-budget").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: u32::try_from(bytes.len() + 1024).unwrap(),
        };
        inventory.observe(observation).unwrap();
        assert_eq!(
            inventory.candidates[0].observation.advertisement,
            advertised
        );
    }
}
