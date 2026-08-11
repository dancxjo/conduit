use std::path::Path;

use conduit_core::HostAdvertisement;
use conduit_observatory::{
    CapabilityAvailability, CapabilitySupport, ObservatorySnapshot, OfferFreshness,
};
use serde::Serialize;

use super::{inventory::InventoryEntry, CatalogError};

#[derive(Serialize)]
pub struct ObservedOffer {
    host_id: String,
    boot_id: String,
    offer_generation: u64,
    capability_id: Option<String>,
    freshness: Option<OfferFreshness>,
    support: Option<CapabilitySupport>,
    availability: Option<CapabilityAvailability>,
}

#[derive(Serialize)]
pub struct CurrentOffer {
    status: &'static str,
    observations: Vec<ObservedOffer>,
}

pub fn load(path: &Path) -> Result<ObservatorySnapshot, CatalogError> {
    let bytes = std::fs::read(path)
        .map_err(|error| CatalogError::new("observatory-snapshot-unreadable", error.to_string()))?;
    let snapshot = serde_json::from_slice(&bytes).map_err(|error| {
        CatalogError::new("observatory-snapshot-invalid-json", error.to_string())
    })?;
    conduit_observatory::validate_snapshot(&snapshot)
        .map_err(|error| CatalogError::new("observatory-snapshot-invalid", error))?;
    Ok(snapshot)
}

pub fn current(
    profile: &HostAdvertisement,
    kind: &InventoryEntry,
    snapshot: Option<&ObservatorySnapshot>,
) -> CurrentOffer {
    let Some(snapshot) = snapshot else {
        return CurrentOffer {
            status: "not-observed",
            observations: Vec::new(),
        };
    };
    let observations = snapshot
        .hosts
        .iter()
        .filter(|host| host.advertisement.profile == profile.profile)
        .map(|host| {
            let capability = host.advertisement.capabilities.iter().find(|capability| {
                capability.kind_id.as_str() == kind.kind_id
                    && capability.kind_contract_revision.as_str() == kind.contract_revision
            });
            let status = capability.and_then(|capability| {
                host.capabilities
                    .iter()
                    .find(|status| status.capability_id == capability.capability_id)
            });
            ObservedOffer {
                host_id: host.advertisement.host_id.as_str().to_owned(),
                boot_id: host.advertisement.boot_id.as_str().to_owned(),
                offer_generation: host.advertisement.offer_generation.0,
                capability_id: capability.map(|value| value.capability_id.as_str().to_owned()),
                freshness: status.map(|value| value.freshness),
                support: status.map(|value| value.support),
                availability: status.map(|value| value.availability),
            }
        })
        .collect();
    CurrentOffer {
        status: "observed",
        observations,
    }
}
