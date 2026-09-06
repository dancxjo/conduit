//! Membership advertises the installed local catalog as well as live webchat.
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
        assert!(advertised.capabilities.len() <= conduit_body::MAX_CANDIDATE_CAPABILITIES);
        assert!(
            bytes.len() + 1024 <= conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize,
            "installed membership advertisement: {} bytes, {} capabilities",
            bytes.len(),
            advertised.capabilities.len()
        );
    }
}
