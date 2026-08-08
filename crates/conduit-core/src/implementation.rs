use serde::{Deserialize, Serialize};

use crate::{
    ArtifactId, BootId, CapabilityId, ExecutionProfileId, HostId, ImplementationId,
    OfferGeneration, RealizationCharacteristicId,
};
use alloc::string::String;
use alloc::vec::Vec;

/// One exact executable realization offered beneath a semantic capability face.
///
/// These are stable realization facts. Current availability and utilization are
/// deliberately not part of this value and belong to planner observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationOffer {
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RealizationCharacteristicValue {
    Count(u64),
    Flag(bool),
    Label(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RealizationCharacteristic {
    pub characteristic_id: RealizationCharacteristicId,
    pub value: RealizationCharacteristicValue,
}

/// Stable, exact facts advertised alongside one boot-scoped implementation offer.
/// Current availability belongs in `ResourceObservation`, not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizationAdvertisement {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub characteristics: Vec<RealizationCharacteristic>,
}
