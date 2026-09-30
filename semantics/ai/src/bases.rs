use crate::{
    llm_contract, BaseProofClass, DataHandling, Metering, LLM_GENERATE_KIND, LOCAL_MODEL_OPERATION,
    MAXIMUM_LLM_INPUT_BYTES,
};
use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{
    compute_resource_offer, compute_resource_requirement, kind_id, resource_offer,
    resource_requirement, stable_realization_boolean, stable_realization_quantity,
    ArchitectureBaseId, ArchitectureBaseKind, ArtifactId, AuthorityContractId,
    AuthorityRequirement, BootId, CapabilityId, CapabilityLimits, CharacteristicUnit,
    ComputePoolContract, ComputeServiceGuarantee, ExecutionProfileId, HostAdvertisement,
    HostCallContractId, HostCallRequirement, HostId, HostProfileId, ImplementationId,
    ImplementationOffer, OfferGeneration, RealizationAdvertisement, RealizationCharacteristic,
};
use serde::{Deserialize, Serialize};

pub const CPU_EXECUTION_RESOURCE: &str = "conduit.resource/compute/shared-lane@1";
pub const HOST_MEMORY_GIB_RESOURCE: &str = "conduit.resource/memory/gib@1";
pub const ACCELERATOR_SLOT_RESOURCE: &str = "conduit.resource/accelerator/slot@1";
pub const ACCELERATOR_MEMORY_GIB_RESOURCE: &str = "conduit.resource/accelerator-memory/gib@1";
pub const NETWORK_EGRESS_RESOURCE: &str = "conduit.resource/network-egress/slot@1";
pub const INFERENCE_SLOT_RESOURCE: &str = "conduit.resource/inference/slot@1";
pub const SMALL_LOCAL_IMPLEMENTATION: &str = "ai.fixture/small-local-cpu@1";
pub const SMALL_LOCAL_ARTIFACT: &str = "ai.fixture/small-local-cpu/x86_64-portable@1";
pub const LARGE_LOCAL_IMPLEMENTATION: &str = "ai.fixture/large-local-accelerated@1";
pub const LARGE_LOCAL_ARTIFACT: &str = "ai.fixture/large-local-accelerated/x86_64-accelerator@1";
pub const REMOTE_FRONTIER_IMPLEMENTATION: &str = "ai.fixture/remote-frontier@1";
pub const REMOTE_FRONTIER_ARTIFACT: &str = "ai.fixture/remote-frontier/host-call@1";
pub const REMOTE_LLM_AUTHORITY: &str = "conduit.authority/remote-llm@1";
pub const MAXIMUM_CONTEXT_CHARACTERISTIC: &str = "conduit.realization/maximum-context-items@1";
pub const MAXIMUM_OUTPUT_CHARACTERISTIC: &str = "conduit.realization/maximum-output-bytes@1";
pub const DATA_EGRESS_CHARACTERISTIC: &str = "conduit.realization/data-egress@1";
pub const METERED_COST_CHARACTERISTIC: &str = "conduit.realization/metered-cost@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmGenerateBaseFacts {
    pub proof_class: BaseProofClass,
    pub maximum_context_items: u64,
    pub maximum_output_bytes: u64,
    pub data_handling: DataHandling,
    pub metering: Metering,
    pub benchmark_sign: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmGenerateBaseFixture {
    pub advertisement: HostAdvertisement,
    pub facts: LlmGenerateBaseFacts,
}

pub fn llm_generate_base_fixtures() -> [LlmGenerateBaseFixture; 3] {
    [small_local(), large_local(), remote_frontier()]
}

pub fn llm_generate_realization_advertisements(
    fixtures: &[LlmGenerateBaseFixture],
) -> alloc::vec::Vec<RealizationAdvertisement> {
    fixtures
        .iter()
        .map(|fixture| {
            let offer = &fixture.advertisement.capabilities[0];
            RealizationAdvertisement {
                host_id: fixture.advertisement.host_id.clone(),
                boot_id: fixture.advertisement.boot_id.clone(),
                offer_generation: fixture.advertisement.offer_generation,
                capability_id: offer.capability_id.clone(),
                characteristics: vec![
                    count_characteristic(
                        MAXIMUM_CONTEXT_CHARACTERISTIC,
                        fixture.facts.maximum_context_items,
                        CharacteristicUnit::Items,
                    ),
                    count_characteristic(
                        MAXIMUM_OUTPUT_CHARACTERISTIC,
                        fixture.facts.maximum_output_bytes,
                        CharacteristicUnit::Bytes,
                    ),
                    flag_characteristic(
                        DATA_EGRESS_CHARACTERISTIC,
                        fixture.facts.data_handling == DataHandling::RemoteEgress,
                    ),
                    flag_characteristic(
                        METERED_COST_CHARACTERISTIC,
                        fixture.facts.metering == Metering::MeteredFixture,
                    ),
                ],
            }
        })
        .collect()
}

fn count_characteristic(
    id: &str,
    value: u64,
    unit: CharacteristicUnit,
) -> RealizationCharacteristic {
    stable_realization_quantity(
        id,
        id,
        "Stable reviewed LLM realization ceiling in the named semantic unit.",
        unit,
        u64::MAX,
        value,
    )
}

fn flag_characteristic(id: &str, value: bool) -> RealizationCharacteristic {
    stable_realization_boolean(id, id, "Stable reviewed LLM realization behavior.", value)
}

fn small_local() -> LlmGenerateBaseFixture {
    base(
        "ai-small-local",
        "ai-small-local-boot",
        "ai/small-local",
        SMALL_LOCAL_IMPLEMENTATION,
        SMALL_LOCAL_ARTIFACT,
        &[
            (CPU_EXECUTION_RESOURCE, 3),
            (HOST_MEMORY_GIB_RESOURCE, 6),
            (INFERENCE_SLOT_RESOURCE, 1),
        ],
        8,
        8_192,
        DataHandling::LocalOnly,
        Metering::UnmeteredFixture,
        "fixture/quality-small-v1",
        false,
    )
}

fn large_local() -> LlmGenerateBaseFixture {
    base(
        "ai-large-local",
        "ai-large-local-boot",
        "ai/large-local",
        LARGE_LOCAL_IMPLEMENTATION,
        LARGE_LOCAL_ARTIFACT,
        &[
            (CPU_EXECUTION_RESOURCE, 2),
            (HOST_MEMORY_GIB_RESOURCE, 12),
            (ACCELERATOR_SLOT_RESOURCE, 1),
            (ACCELERATOR_MEMORY_GIB_RESOURCE, 22),
            (INFERENCE_SLOT_RESOURCE, 1),
        ],
        32,
        32_768,
        DataHandling::LocalOnly,
        Metering::UnmeteredFixture,
        "fixture/quality-large-v1",
        false,
    )
}

fn remote_frontier() -> LlmGenerateBaseFixture {
    base(
        "ai-remote-base",
        "ai-remote-base-boot",
        "ai/remote-frontier",
        REMOTE_FRONTIER_IMPLEMENTATION,
        REMOTE_FRONTIER_ARTIFACT,
        &[
            (CPU_EXECUTION_RESOURCE, 1),
            (HOST_MEMORY_GIB_RESOURCE, 1),
            (NETWORK_EGRESS_RESOURCE, 1),
            (INFERENCE_SLOT_RESOURCE, 1),
        ],
        128,
        65_536,
        DataHandling::RemoteEgress,
        Metering::MeteredFixture,
        "fixture/quality-remote-v1",
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn base(
    host: &str,
    boot: &str,
    capability: &str,
    implementation: &str,
    artifact: &str,
    resources: &[(&str, u32)],
    maximum_context_items: u64,
    maximum_output_bytes: u64,
    data_handling: DataHandling,
    metering: Metering,
    benchmark_sign: &str,
    remote: bool,
) -> LlmGenerateBaseFixture {
    let kind = llm_contract(LLM_GENERATE_KIND)
        .expect("generation contract is catalogued")
        .into_capability_contract();
    let semantic_contract = kind.semantic_contract();
    let host_call = HostCallRequirement {
        contract_id: HostCallContractId::from(LOCAL_MODEL_OPERATION),
        target_kind: Some(kind_id(LLM_GENERATE_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_LLM_INPUT_BYTES as u32,
        maximum_output_bytes: maximum_output_bytes as u32,
    };
    let capability_id = CapabilityId::from(capability);
    let authority_requirements = if remote {
        vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(REMOTE_LLM_AUTHORITY),
            host_call_contract_id: host_call.contract_id.clone(),
            subject_kind: kind_id(LLM_GENERATE_KIND),
        }]
    } else {
        vec![]
    };
    let mut resource_offers = resources
        .iter()
        .enumerate()
        .map(|(index, (class, units))| {
            if *class == CPU_EXECUTION_RESOURCE {
                compute_resource_offer(
                    &format_pool(host, index),
                    class,
                    *units,
                    ComputePoolContract {
                        service_guarantee: ComputeServiceGuarantee::Shared,
                        architecture_base_id: ArchitectureBaseId::from(alloc::format!(
                            "fixture/{host}/hosted-os-cpu"
                        )),
                        architecture_base_kind: ArchitectureBaseKind::HostedOs,
                        topology_groups: vec![],
                    },
                )
            } else {
                resource_offer(&format_pool(host, index), class, *units)
            }
        })
        .collect::<alloc::vec::Vec<_>>();
    resource_offers.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let mut resource_requirements = resources
        .iter()
        .map(|(class, units)| {
            if *class == CPU_EXECUTION_RESOURCE {
                compute_resource_requirement(
                    class,
                    1,
                    *units,
                    units.saturating_mul(2),
                    ComputeServiceGuarantee::Shared,
                    None,
                )
            } else {
                resource_requirement(class, *units)
            }
        })
        .collect::<alloc::vec::Vec<_>>();
    resource_requirements.sort();
    LlmGenerateBaseFixture {
        advertisement: HostAdvertisement {
            protocol_version: 1,
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("conduit.host/fixture@1"),
            bases: vec![],
            resources: resource_offers,
            capabilities: vec![conduit_core::capability_offer_from_parts! {
                startup_parameters: kind.startup_parameters,
                shorthand: kind.shorthand,
                capability_id,
                kind_id: kind.kind_id,
                kind_contract_revision: kind.kind_contract_revision,
                inputs: kind.inputs,
                outputs: kind.outputs,
                semantic_contract,
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from(
                        "conduit.llm/generate-fixture@1",
                    ),
                    implementation_id: ImplementationId::from(implementation),
                    artifact_id: ArtifactId::from(artifact),
                },
                host_calls: vec![host_call],
                resource_requirements,
                authority_requirements,
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: (MAXIMUM_LLM_INPUT_BYTES + maximum_output_bytes) as u32,
                },
            }],
            planner_capabilities: vec![],
        },
        facts: LlmGenerateBaseFacts {
            proof_class: BaseProofClass::DeterministicConformanceFixture,
            maximum_context_items,
            maximum_output_bytes,
            data_handling,
            metering,
            benchmark_sign: benchmark_sign.to_string(),
        },
    }
}

fn format_pool(host: &str, index: usize) -> String {
    alloc::format!("{host}/resource-{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_materially_different_fixtures_offer_one_equal_checked_front() {
        let fixtures = llm_generate_base_fixtures();
        let front = fixtures[0].advertisement.capabilities[0].checked_front();
        for fixture in &fixtures {
            assert_eq!(fixture.advertisement.capabilities[0].checked_front(), front);
            assert_eq!(
                fixture.facts.proof_class,
                BaseProofClass::DeterministicConformanceFixture
            );
        }
        assert_ne!(fixtures[0].facts, fixtures[1].facts);
        assert_ne!(fixtures[1].facts, fixtures[2].facts);
        assert!(fixtures[0].advertisement.capabilities[0]
            .authority_requirements
            .is_empty());
        assert_eq!(
            fixtures[2].advertisement.capabilities[0]
                .authority_requirements
                .len(),
            1
        );
        assert!(fixtures[2].advertisement.capabilities[0]
            .resource_requirements
            .iter()
            .any(|requirement| requirement.class_id.as_str() == NETWORK_EGRESS_RESOURCE));
    }

    #[test]
    fn implementation_artifact_and_resource_vectors_are_exact_and_distinct() {
        let fixtures = llm_generate_base_fixtures();
        for pair in fixtures.windows(2) {
            let left = &pair[0].advertisement.capabilities[0];
            let right = &pair[1].advertisement.capabilities[0];
            assert_ne!(
                left.implementation.implementation_id,
                right.implementation.implementation_id
            );
            assert_ne!(
                left.implementation.artifact_id,
                right.implementation.artifact_id
            );
            assert_ne!(left.resource_requirements, right.resource_requirements);
        }
    }
}
