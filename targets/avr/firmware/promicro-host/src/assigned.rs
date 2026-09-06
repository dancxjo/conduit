use conduit_assigned_plan::{
    decode_assigned_plan, AssignedPlanMaxima, AssignedPlanRefusal, AssignedPlanRequirements,
    AssignedPlanView,
};

/// Validate the ordinary compact projection selected for this exact Host.
///
/// Transport and storage are board mechanisms; the schema, identities,
/// inventory checks, bounds, and refusal meanings remain generic Conduit.
pub fn validate(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    decode_assigned_plan(bytes, maxima, requirements)
}
