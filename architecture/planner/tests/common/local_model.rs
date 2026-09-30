use conduit_ai::{
    LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelComputeNeed,
    LocalModelIdentity, LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits,
    LocalModelOffer, LOCAL_MODEL_COMPUTE_RESOURCE, LOCAL_MODEL_INFERENCE_SLOT_RESOURCE,
    LOCAL_MODEL_MEMORY_RESOURCE, LOCAL_MODEL_QUEUE_ITEM_RESOURCE, LOCAL_MODEL_QUEUE_KIB_RESOURCE,
};
use conduit_core::{
    compute_resource_offer, resource_offer, ArchitectureBaseId, ArchitectureBaseKind, BootId,
    ComputePoolContract, ComputeServiceGuarantee, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration,
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LocalModelProviderFixture {
    pub provider: LocalModelOffer,
    pub advertisement: HostAdvertisement,
}

#[allow(dead_code)]
pub fn dual_local_model_providers() -> [LocalModelProviderFixture; 2] {
    [
        local_model_provider("compact", 6, (2, 4, 6), LocalModelKindProfile::Generate),
        local_model_provider("wide", 16, (4, 8, 12), LocalModelKindProfile::Generate),
    ]
}

#[allow(dead_code)]
pub fn local_model_provider(
    id: &str,
    lanes: u32,
    need: (u32, u32, u32),
    profile: LocalModelKindProfile,
) -> LocalModelProviderFixture {
    let provider = LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "fixture-runtime".into(),
            runtime_version: "1".into(),
            runtime_build_identity: format!("runtime/{id}"),
            model_name: id.into(),
            model_content_identity: format!("sha256-{id}"),
            architecture: "transformer".into(),
            parameter_profile: "bounded".into(),
            quantization: "fixture".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds {
                maximum_input_bytes: 4_096,
                maximum_context_items: 1,
                maximum_output_bytes: 1_024,
                maximum_work_units: 4_096,
                maximum_history_items: 0,
            },
            model_bytes: 1,
            admitted_memory_mib: 8,
            compute: LocalModelComputeNeed {
                minimum_lanes: need.0,
                preferred_lanes: need.1,
                maximum_lanes: need.2,
                minimum_service_guarantee: ComputeServiceGuarantee::Shared,
            },
            maximum_in_flight: 1,
            maximum_queue_items: 2,
            maximum_queue_bytes: 8_192,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![profile],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    };
    let construction = format!(
        "host {id} {{\n  schema = 1\n  target = {{architecture: \"x86_64\", machine: \"workstation\", os: \"linux\"}}\n  need = {{id: \"{id}/memory\", class: \"{LOCAL_MODEL_MEMORY_RESOURCE}\", slots: 8, bytes: 1}}\n  need = {{id: \"{id}/compute\", class: \"{LOCAL_MODEL_COMPUTE_RESOURCE}\", slots: {lanes}, bytes: 1}}\n  need = {{id: \"{id}/slot\", class: \"{LOCAL_MODEL_INFERENCE_SLOT_RESOURCE}\", slots: 1, bytes: 1}}\n  need = {{id: \"{id}/queue-items\", class: \"{LOCAL_MODEL_QUEUE_ITEM_RESOURCE}\", slots: 2, bytes: 1}}\n  need = {{id: \"{id}/queue-kib\", class: \"{LOCAL_MODEL_QUEUE_KIB_RESOURCE}\", slots: 8, bytes: 1}}\n  limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}\n}}\n"
    );
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&construction).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let mut resources = checked
        .configuration()
        .resources
        .iter()
        .map(|budget| {
            if budget.class == LOCAL_MODEL_COMPUTE_RESOURCE {
                compute_resource_offer(
                    &budget.id,
                    &budget.class,
                    budget.slots,
                    ComputePoolContract {
                        service_guarantee: ComputeServiceGuarantee::Shared,
                        architecture_base_id: ArchitectureBaseId::from(format!(
                            "{id}/hosted-compute"
                        )),
                        architecture_base_kind: ArchitectureBaseKind::HostedOs,
                        topology_groups: vec![],
                    },
                )
            } else {
                resource_offer(&budget.id, &budget.class, budget.slots)
            }
        })
        .collect::<Vec<_>>();
    resources.sort();
    let advertisement = HostAdvertisement {
        protocol_version: 1,
        host_id: HostId::from(format!("host/{id}")),
        boot_id: BootId::from(format!("boot/{id}/1")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduit.host/local-model-fixture@1"),
        bases: vec![],
        resources,
        capabilities: provider.capability_offers().unwrap(),
        planner_capabilities: vec![],
    };
    LocalModelProviderFixture {
        provider,
        advertisement,
    }
}
