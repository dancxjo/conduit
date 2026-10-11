use crate::prelude::*;
use crate::PlacementChoice;
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CharacteristicId, GearId, HostId, ImplementationId,
    OfferGeneration,
};

pub const MAXIMUM_REALIZATION_DECISION_RECORDS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationRejection {
    LanguageCoverage {
        request: conduit_language::LanguageRequest,
        reason: conduit_language::LanguageCoverageRefusal,
    },
    QueueItemBound,
    QueueByteBound,
    ResourceUnitCeiling,
    HostCallAllowlist,
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

/// Bounded, prompt-free planning signs for one equal-front candidate.
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

/// A failed ordinary selection with its bounded candidate account, when hard
/// admission reached candidate evaluation. Invalid inputs have no such account.
#[derive(Debug)]
pub struct RealizationSelectionRefusal {
    pub reason: crate::PlannerError,
    pub signs: Vec<RealizationDecisionRecord>,
}
impl From<crate::PlannerError> for RealizationSelectionRefusal {
    fn from(reason: crate::PlannerError) -> Self {
        Self {
            reason,
            signs: Vec::new(),
        }
    }
}

/// Compatibility entry retaining the ordinary error contract.
pub fn select_realization_with_characteristics_and_signs(
    gear: &conduit_plot::CheckedGear,
    hosts: &[conduit_core::HostAdvertisement],
    advertisements: &[conduit_core::RealizationAdvertisement],
    requirements: &crate::HardRealizationRequirements,
    observations: &[conduit_core::ResourceObservation],
    policy: &crate::RealizationPolicy,
) -> Result<RealizationSelection, crate::PlannerError> {
    crate::select_realization_with_characteristics_account(
        gear,
        hosts,
        advertisements,
        requirements,
        observations,
        policy,
    )
    .map_err(|refusal| refusal.reason)
}
