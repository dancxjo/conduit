//! Exact hosted realization of the bounded Todo combine Kind.

use conduit_core::{
    kind_id, ArtifactId, AuthorityRequirement, Back, BackOfferBuilder, CapabilityId,
    CapabilityOffer, ExecutionProfileId, HostCallRequirement, ImplementationId, ResourceAccessMode,
    ResourceContentRequirement, ResourceRequirement, ResourceRetention, ResourceSharing,
};

pub const TODO_COMBINE_IMPLEMENTATION: &str = "std/kernel-todo-combine@1";
pub const TODO_COMBINE_ARTIFACT: &str = "conduit-std-host/todo-combine@1";
pub const TODO_COMBINE_PROFILE: &str = "conduit.std/todo-combine-bounded@1";
pub const TODO_CHECKPOINT_IMPLEMENTATION: &str = "std/kernel-todo-checkpoint@1";
pub const TODO_CHECKPOINT_ARTIFACT: &str = "conduit-std-host/todo-checkpoint@1";
pub const TODO_CHECKPOINT_PROFILE: &str = "conduit.std/todo-checkpoint@1";
pub const TODO_CHECKPOINT_PUBLISH_CALL: &str = "conduit.host/todo-checkpoint-publish@1";
pub const TODO_CHECKPOINT_AUTHORITY: &str = "authority/todo-checkpoint@1";

/// One externally durable immutable generation selected for one Todo command.
pub fn todo_checkpoint_offer(
    contract: ResourceContentRequirement,
) -> Result<CapabilityOffer, &'static str> {
    contract
        .validate()
        .map_err(|_| "invalid Todo checkpoint content contract")?;
    if contract.access != ResourceAccessMode::WriteCandidatePublish
        || contract.retention != ResourceRetention::ExternalDurable
        || contract.sharing != ResourceSharing::SingleWriterPublished
        || contract.content_profile != kind_id("conduit.todo/checkpoint-envelope@1")
        || contract.maximum_bytes != 2135
        || contract.maximum_items != 1
        || contract.generation_slots != 1
        || contract.publication_slots != 1
        || contract.reader_leases != 1
        || contract.sensitive
    {
        return Err("unsupported Todo checkpoint content contract");
    }
    let kind = conduit_todo_plot::todo_checkpoint_kind();
    let kind_id = kind.kind_id.clone();
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from("std-todo-checkpoint-v1"),
            execution_profile_id: ExecutionProfileId::from(TODO_CHECKPOINT_PROFILE),
            implementation_id: ImplementationId::from(TODO_CHECKPOINT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TODO_CHECKPOINT_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: TODO_CHECKPOINT_PUBLISH_CALL.into(),
                target_kind: Some(kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 4096,
                maximum_output_bytes: 4096,
            }],
            resource_requirements: vec![ResourceRequirement {
                class_id: "resource/todo-checkpoint@1".into(),
                units: 1,
                compute: None,
                protected_role: None,
                content: Some(contract),
            }],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: TODO_CHECKPOINT_AUTHORITY.into(),
                host_call_contract_id: TODO_CHECKPOINT_PUBLISH_CALL.into(),
                subject_kind: kind_id,
            }],
        },
    )
    .build())
}

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
