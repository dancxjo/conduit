use crate::prelude::*;
use crate::PlacementChoice;
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CharacteristicId, GearId, HostId, ImplementationId,
    OfferGeneration,
};

pub const MAXIMUM_REALIZATION_DECISION_RECORDS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationRejection {
    QueueItemBound,
    QueueByteBound,
    ResourceUnitCeiling,
    HostOperationAllowlist,
    AuthorityContractAllowlist,
    MinimumCharacteristicCount(CharacteristicId),
    MaximumCharacteristicCount(CharacteristicId),
    RequiredCharacteristicFlag(CharacteristicId),
    RequiredCharacteristicLabel(CharacteristicId),
    CurrentResourceObservation,
    HardPredicate {
        clause_index: u16,
        fact: crate::PlannerFactRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationDecisionDisposition {
    Rejected(RealizationRejection),
    Admitted,
    Selected,
}

/// Bounded, prompt-free planning signs for one equal-face candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationDecisionRecord {
    pub gear_id: GearId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub disposition: RealizationDecisionDisposition,
    /// Zero-based first decisive soft clause for the selected candidate. Values
    /// remain in typed planning inputs rather than being copied into evidence.
    pub decisive_preference_clause: Option<u16>,
    /// Source revision for the hard rejection, when selection used explicit
    /// scoped composition. The rejected value remains redacted.
    pub clause_source: Option<crate::PolicySourceRevision>,
    /// Source revision for the decisive soft clause, when scoped composition
    /// supplied that clause.
    pub decisive_preference_source: Option<crate::PolicySourceRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationSelection {
    pub choice: PlacementChoice,
    pub signs: Vec<RealizationDecisionRecord>,
}
