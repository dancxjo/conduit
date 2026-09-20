use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId, Kind,
};

pub const RECURRENCE_STD_PROFILE: &str = "std/recurrence-kernel@1";
pub const RECURRENCE_STD_IMPLEMENTATION: &str = "std/kernel-expand-recurrence@1";
pub const RECURRENCE_STD_ARTIFACT: &str = "conduit-std-host/expand-recurrence@1";
pub const CALENDAR_PROPOSAL_STD_PROFILE: &str = "std/calendar-proposal-kernel@1";
pub const CALENDAR_PROPOSAL_STD_IMPLEMENTATION: &str = "std/kernel-calendar-proposal@1";
pub const CALENDAR_PROPOSAL_STD_ARTIFACT: &str = "conduit-std-host/calendar-proposal@1";

pub fn recurrence_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::recurrence_semantic_contract(),
        "time-expand-recurrence",
        RECURRENCE_STD_PROFILE,
        RECURRENCE_STD_IMPLEMENTATION,
        RECURRENCE_STD_ARTIFACT,
    )
}

pub fn calendar_proposal_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::calendar_proposal_semantic_contract(),
        "calendar-propose-meeting",
        CALENDAR_PROPOSAL_STD_PROFILE,
        CALENDAR_PROPOSAL_STD_IMPLEMENTATION,
        CALENDAR_PROPOSAL_STD_ARTIFACT,
    )
}

fn offer(
    contract: Kind,
    capability: &str,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(execution_profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculation_offers_preserve_exact_portable_shapes_and_bounds() {
        for offer in [recurrence_std_offer(), calendar_proposal_std_offer()] {
            assert_eq!(offer.startup_parameters.len(), 1);
            assert!(offer.inputs.is_empty());
            assert_eq!(offer.outputs.len(), 1);
            assert!(offer.host_operations.is_empty());
            assert!(offer.resource_requirements.is_empty());
            assert!(offer.authority_requirements.is_empty());
            assert!(offer.limits.max_queue_items > 0);
            assert!(offer.limits.max_queue_bytes > 0);
        }
    }
}
