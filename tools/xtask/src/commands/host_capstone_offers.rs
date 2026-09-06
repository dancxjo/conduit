//! Exact Presenter offers and live-fact fixtures for the capstone Hosts.

use conduit_core::{
    kind_id, resource_requirement, ArtifactId, CapabilityId, CapabilityLimits, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId,
};
use conduit_host_fabrication::RuntimeFacts;
use conduit_presentation::{renderer_offer, RendererRealizationOffer, MAX_RENDERER_VALUE_BYTES};

pub(super) fn presenter_offer(
    capability: &str,
    execution: &str,
    implementation: &str,
    artifact: &str,
    target: &str,
) -> conduit_core::CapabilityOffer {
    renderer_offer(RendererRealizationOffer {
        capability_id: CapabilityId::from(capability),
        execution_profile_id: ExecutionProfileId::from(execution),
        implementation_id: ImplementationId::from(implementation),
        artifact_id: ArtifactId::from(artifact),
        host_operation: HostOperationRequirement {
            contract_id: HostOperationContractId::from("conduit.host/present@1"),
            target_kind: Some(kind_id(target)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
            maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
        },
        resource_requirement: resource_requirement("presentation/surface", 1),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
        },
    })
}

pub(super) fn native_facts(ready: bool) -> RuntimeFacts {
    RuntimeFacts {
        ready_resource_classes: ready
            .then(|| "presentation/surface".into())
            .into_iter()
            .collect(),
        initialized_base_kinds: ready
            .then(|| "display/scanout".into())
            .into_iter()
            .collect(),
        initialized_driver_kinds: ready
            .then(|| "display/linear-framebuffer@1".into())
            .into_iter()
            .collect(),
        available_facilities: ready
            .then(|| "compositor/native@1".into())
            .into_iter()
            .collect(),
        authority_ready: false,
    }
}

pub(super) fn browser_facts(ready: bool) -> RuntimeFacts {
    RuntimeFacts {
        ready_resource_classes: ready
            .then(|| "presentation/surface".into())
            .into_iter()
            .collect(),
        initialized_base_kinds: ready.then(|| "browser/dom".into()).into_iter().collect(),
        initialized_driver_kinds: ready.then(|| "browser/dom@1".into()).into_iter().collect(),
        available_facilities: Default::default(),
        authority_ready: false,
    }
}
