//! Finite std-host realization for the portable reaction-diffusion contract.

use conduit_alife::{
    reaction_diffusion_inputs, reaction_diffusion_outputs, ReactionDiffusionEvolveRequest,
    ReactionDiffusionFieldState, ReactionDiffusionRefusal, REACTION_DIFFUSION_EVOLVE_KIND,
    REACTION_DIFFUSION_KIND_REVISION, REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
    REACTION_DIFFUSION_REQUEST_BYTES,
};
use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision,
};

pub const REACTION_DIFFUSION_HOSTED_PROFILE: &str = "std/field-gray-scott-hosted@1";
pub const REACTION_DIFFUSION_HOSTED_ARTIFACT: &str = "conduit-std-host/field-gray-scott@1";
pub const REACTION_DIFFUSION_HOST_OPERATION: &str = "conduit.host/field-evolve@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct HostedReactionDiffusionLimits {
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
    pub maximum_active_instances: u16,
    pub maximum_queued_requests: u16,
}

pub const HOSTED_REACTION_DIFFUSION_LIMITS: HostedReactionDiffusionLimits =
    HostedReactionDiffusionLimits {
        maximum_input_bytes: REACTION_DIFFUSION_MAXIMUM_STATE_BYTES
            + REACTION_DIFFUSION_REQUEST_BYTES,
        maximum_output_bytes: REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
        maximum_active_instances: 1,
        maximum_queued_requests: 1,
    };

pub fn reaction_diffusion_std_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("std/field-evolve@1"),
        kind_id: kind_id(REACTION_DIFFUSION_EVOLVE_KIND),
        kind_contract_revision: KindContractRevision::from(REACTION_DIFFUSION_KIND_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(REACTION_DIFFUSION_HOSTED_PROFILE),
            implementation_id: ImplementationId::from("std/field-gray-scott@1"),
            artifact_id: ArtifactId::from(REACTION_DIFFUSION_HOSTED_ARTIFACT),
        },
        inputs: reaction_diffusion_inputs(),
        outputs: reaction_diffusion_outputs(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(REACTION_DIFFUSION_HOST_OPERATION),
            target_kind: Some(kind_id(REACTION_DIFFUSION_EVOLVE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_input_bytes,
            maximum_output_bytes: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_output_bytes,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_active_instances,
            max_queue_items: HOSTED_REACTION_DIFFUSION_LIMITS.maximum_queued_requests,
            max_queue_bytes: REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
        },
    }
}

/// Executes the reviewed synchronous toroidal ppm profile.
pub fn evolve_reaction_diffusion_hosted(
    state: &ReactionDiffusionFieldState,
    request: ReactionDiffusionEvolveRequest,
) -> Result<ReactionDiffusionFieldState, ReactionDiffusionRefusal> {
    state.evolve_reference(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_alife::{GrayScottParameters, ReactionDiffusionFieldId};

    #[test]
    fn offer_matches_portable_contract_with_finite_hosted_limits() {
        let definition = conduit_alife::reaction_diffusion_definition();
        let offer = reaction_diffusion_std_offer();
        assert_eq!(offer.kind_id, definition.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            definition.kind_contract_revision
        );
        assert_eq!(offer.inputs, definition.inputs);
        assert_eq!(offer.outputs, definition.outputs);
        assert_eq!(offer.limits.max_active_instances, 1);
        assert_eq!(offer.limits.max_queue_items, 1);
        assert_eq!(offer.host_operations.len(), 1);
    }

    #[test]
    fn hosted_reference_is_repeatable() {
        let field_id = ReactionDiffusionFieldId(*b"field-a0-hosted1");
        let state = ReactionDiffusionFieldState::initialized(
            field_id,
            12,
            10,
            GrayScottParameters::REFERENCE,
            42,
        )
        .unwrap();
        let request = ReactionDiffusionEvolveRequest {
            field_id,
            expected_generation: 0,
            generations: 3,
            admitted_cell_generations: 360,
        };
        assert_eq!(
            evolve_reaction_diffusion_hosted(&state, request),
            evolve_reaction_diffusion_hosted(&state, request)
        );
    }
}
