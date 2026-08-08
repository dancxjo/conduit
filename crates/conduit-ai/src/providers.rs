use crate::{generate_text_contract, GENERATE_TEXT_KIND, MAXIMUM_INPUT_BYTES};
use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId,
    AuthorityRequirement, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, ImplementationOffer,
    OfferGeneration,
};
use serde::{Deserialize, Serialize};

pub const CPU_EXECUTION_RESOURCE: &str = "conduit.resource/compute/shared-lane@1";
pub const HOST_MEMORY_GIB_RESOURCE: &str = "conduit.resource/memory/gib@1";
pub const ACCELERATOR_SLOT_RESOURCE: &str = "conduit.resource/accelerator/slot@1";
pub const ACCELERATOR_MEMORY_GIB_RESOURCE: &str = "conduit.resource/accelerator-memory/gib@1";
pub const NETWORK_EGRESS_RESOURCE: &str = "conduit.resource/network-egress/slot@1";
pub const INFERENCE_SLOT_RESOURCE: &str = "conduit.resource/inference/slot@1";
pub const GENERATE_TEXT_HOST_OPERATION: &str = "conduit.host/generate-text@1";
pub const REMOTE_GENERATE_TEXT_AUTHORITY: &str = "conduit.authority/remote-generate-text@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderProofClass {
    DeterministicConformanceFixture,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataHandling {
    LocalOnly,
    RemoteEgress,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metering {
    UnmeteredFixture,
    MeteredFixture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateTextProviderFacts {
    pub proof_class: ProviderProofClass,
    pub maximum_context_tokens: u64,
    pub maximum_output_tokens: u64,
    pub data_handling: DataHandling,
    pub metering: Metering,
    pub benchmark_evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateTextProviderFixture {
    pub advertisement: HostAdvertisement,
    pub facts: GenerateTextProviderFacts,
}

pub fn generate_text_provider_fixtures() -> [GenerateTextProviderFixture; 3] {
    [small_local(), large_local(), remote_frontier()]
}

fn small_local() -> GenerateTextProviderFixture {
    provider(
        "ai-small-local",
        "ai-small-local-boot",
        "ai/small-local",
        "ai.fixture/small-local-cpu@1",
        "ai.fixture/small-local-cpu/x86_64-portable@1",
        &[
            (CPU_EXECUTION_RESOURCE, 3),
            (HOST_MEMORY_GIB_RESOURCE, 6),
            (INFERENCE_SLOT_RESOURCE, 1),
        ],
        8_192,
        2_048,
        DataHandling::LocalOnly,
        Metering::UnmeteredFixture,
        "fixture/quality-small-v1",
        false,
    )
}

fn large_local() -> GenerateTextProviderFixture {
    provider(
        "ai-large-local",
        "ai-large-local-boot",
        "ai/large-local",
        "ai.fixture/large-local-accelerated@1",
        "ai.fixture/large-local-accelerated/x86_64-accelerator@1",
        &[
            (CPU_EXECUTION_RESOURCE, 2),
            (HOST_MEMORY_GIB_RESOURCE, 12),
            (ACCELERATOR_SLOT_RESOURCE, 1),
            (ACCELERATOR_MEMORY_GIB_RESOURCE, 22),
            (INFERENCE_SLOT_RESOURCE, 1),
        ],
        32_768,
        8_192,
        DataHandling::LocalOnly,
        Metering::UnmeteredFixture,
        "fixture/quality-large-v1",
        false,
    )
}

fn remote_frontier() -> GenerateTextProviderFixture {
    provider(
        "ai-remote-provider",
        "ai-remote-provider-boot",
        "ai/remote-frontier",
        "ai.fixture/remote-frontier@1",
        "ai.fixture/remote-frontier/host-operation@1",
        &[
            (CPU_EXECUTION_RESOURCE, 1),
            (HOST_MEMORY_GIB_RESOURCE, 1),
            (NETWORK_EGRESS_RESOURCE, 1),
            (INFERENCE_SLOT_RESOURCE, 1),
        ],
        200_000,
        16_384,
        DataHandling::RemoteEgress,
        Metering::MeteredFixture,
        "fixture/quality-remote-v1",
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn provider(
    host: &str,
    boot: &str,
    capability: &str,
    implementation: &str,
    artifact: &str,
    resources: &[(&str, u32)],
    maximum_context_tokens: u64,
    maximum_output_tokens: u64,
    data_handling: DataHandling,
    metering: Metering,
    benchmark_evidence: &str,
    remote: bool,
) -> GenerateTextProviderFixture {
    let contract = generate_text_contract();
    let host_operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(GENERATE_TEXT_HOST_OPERATION),
        target_kind: Some(kind_id(GENERATE_TEXT_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_INPUT_BYTES as u32,
        maximum_output_bytes: maximum_output_tokens as u32 * 4,
    };
    let capability_id = CapabilityId::from(capability);
    let authority_requirements = if remote {
        vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(REMOTE_GENERATE_TEXT_AUTHORITY),
            host_operation_contract_id: host_operation.contract_id.clone(),
            subject_kind: kind_id("ai/remote-provider-subject"),
        }]
    } else {
        vec![]
    };
    let resource_offers = resources
        .iter()
        .enumerate()
        .map(|(index, (class, units))| resource_offer(&format_pool(host, index), class, *units))
        .collect();
    let resource_requirements = resources
        .iter()
        .map(|(class, units)| resource_requirement(class, *units))
        .collect();
    GenerateTextProviderFixture {
        advertisement: HostAdvertisement {
            protocol_version: 1,
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("conduit.host/fixture@1"),
            resources: resource_offers,
            capabilities: vec![CapabilityOffer {
                startup_parameters: startup_parameters(),
                shorthand: None,
                capability_id,
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from(
                        "conduit.ai/generate-text-hosted@1",
                    ),
                    implementation_id: ImplementationId::from(implementation),
                    artifact_id: ArtifactId::from(artifact),
                },
                host_operations: vec![host_operation],
                resource_requirements,
                authority_requirements,
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAXIMUM_INPUT_BYTES as u32,
                },
            }],
            planner_capabilities: vec![],
        },
        facts: GenerateTextProviderFacts {
            proof_class: ProviderProofClass::DeterministicConformanceFixture,
            maximum_context_tokens,
            maximum_output_tokens,
            data_handling,
            metering,
            benchmark_evidence: benchmark_evidence.to_string(),
        },
    }
}

fn startup_parameters() -> alloc::vec::Vec<FaceStartupParameter> {
    [
        "maximum-input-bytes",
        "maximum-context-tokens",
        "maximum-output-tokens",
        "temperature-milli",
    ]
    .into_iter()
    .map(|name| FaceStartupParameter {
        name: name.to_string(),
        value_type: "Count".to_string(),
        has_default: true,
    })
    .collect()
}

fn format_pool(host: &str, index: usize) -> String {
    alloc::format!("{host}/resource-{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_materially_different_fixtures_offer_one_equal_checked_face() {
        let fixtures = generate_text_provider_fixtures();
        let face = fixtures[0].advertisement.capabilities[0].checked_face();
        for fixture in &fixtures {
            assert_eq!(fixture.advertisement.capabilities[0].checked_face(), face);
            assert_eq!(
                fixture.facts.proof_class,
                ProviderProofClass::DeterministicConformanceFixture
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
        let fixtures = generate_text_provider_fixtures();
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
