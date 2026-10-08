//! Exact hosted realization of the bounded Todo combine Kind.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId,
};

pub const TODO_COMBINE_IMPLEMENTATION: &str = "std/kernel-todo-combine@1";
pub const TODO_COMBINE_ARTIFACT: &str = "conduit-std-host/todo-combine@1";
pub const TODO_COMBINE_PROFILE: &str = "conduit.std/todo-combine-bounded@1";

pub fn todo_combine_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_todo_plot::todo_combine_kind(),
        Back {
            capability_id: CapabilityId::from("std-todo-combine-v1"),
            execution_profile_id: ExecutionProfileId::from(TODO_COMBINE_PROFILE),
            implementation_id: ImplementationId::from(TODO_COMBINE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TODO_COMBINE_ARTIFACT),
            host_calls: Vec::new(),
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
    fn offer_keeps_exact_todo_kind_and_finite_bounds() {
        let offer = todo_combine_offer();
        let kind = conduit_todo_plot::todo_combine_kind();
        assert_eq!(offer.kind_id, kind.kind_id);
        assert_eq!(offer.kind_contract_revision, kind.kind_contract_revision);
        assert_eq!(offer.inputs, kind.inputs);
        assert_eq!(offer.outputs, kind.outputs);
        assert_eq!(offer.limits, kind.limits);
        assert!(offer.host_calls.is_empty());
    }
}
