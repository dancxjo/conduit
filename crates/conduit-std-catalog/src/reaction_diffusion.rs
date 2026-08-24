//! Finite hosted reference realization for the portable reaction-diffusion contract.

use conduit_core::{
    ReactionDiffusionEvolveRequest, ReactionDiffusionFieldState, ReactionDiffusionRefusal,
    REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
};

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
            + conduit_core::REACTION_DIFFUSION_REQUEST_BYTES,
        maximum_output_bytes: REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
        maximum_active_instances: 1,
        maximum_queued_requests: 1,
    };

/// Executes the reviewed synchronous toroidal ppm profile.
///
/// Both generation buffers are finite from the admitted state dimensions. No
/// history, retry, platform clock, or ambient parameter source participates.
pub fn evolve_reaction_diffusion_hosted(
    state: &ReactionDiffusionFieldState,
    request: ReactionDiffusionEvolveRequest,
) -> Result<ReactionDiffusionFieldState, ReactionDiffusionRefusal> {
    state.evolve_reference(request)
}
