//! Exact text realization offers owned by the hosted std Host.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, Kind,
};

pub const TEXT_LITERAL_EXECUTION_PROFILE: &str = "conduit.std/text-literal-kernel-hosted@1";
pub const TEXT_LITERAL_IMPLEMENTATION: &str = "std/kernel-text-literal@1";
pub const TEXT_LITERAL_ARTIFACT: &str = "conduit-std-host/text-literal@1";
pub const TEXT_LITERAL_CAPABILITY: &str = "text-literal-v1";
pub const TEXT_UPPER_EXECUTION_PROFILE: &str = "conduit.std/text-upper-kernel-hosted@1";
pub const TEXT_UPPER_IMPLEMENTATION: &str = "std/kernel-text-upper@1";
pub const TEXT_UPPER_ARTIFACT: &str = "conduit-std-host/text-upper@1";
pub const TEXT_UPPER_CAPABILITY: &str = "text-upper-v1";
pub const TEXT_UPPER_HOST_OPERATION_CONTRACT: &str = "conduit.host/text-upper@1";
pub const TEXT_UPPER_HOST_OPERATION_TARGET: &str = "text/uppercase-utf8";
pub const TEXT_JOIN_EXECUTION_PROFILE: &str = "conduit.std/text-join-kernel-hosted@1";
pub const TEXT_JOIN_IMPLEMENTATION: &str = "std/kernel-text-join@1";
pub const TEXT_JOIN_ARTIFACT: &str = "conduit-std-host/text-join@1";
pub const TEXT_JOIN_CAPABILITY: &str = "text-join-v1";
pub const TEXT_JOIN_HOST_OPERATION_CONTRACT: &str = "conduit.host/text-join@1";
pub const TEXT_JOIN_HOST_OPERATION_TARGET: &str = "text/prefix-concat-utf8";
pub const TEXT_MORSE_EXECUTION_PROFILE: &str = "conduit.std/text-morse-kernel-hosted@1";
pub const TEXT_MORSE_IMPLEMENTATION: &str = "std/kernel-text-morse@1";
pub const TEXT_MORSE_ARTIFACT: &str = "conduit-std-host/text-morse@1";
pub const TEXT_MORSE_CAPABILITY: &str = "text-morse-v1";
pub const TEXT_MORSE_HOST_OPERATION_CONTRACT: &str = "conduit.host/text-to-morse@1";
pub const TEXT_MORSE_HOST_OPERATION_TARGET: &str = "text/morse-pattern";
pub const ADDRESS_DETECT_EXECUTION_PROFILE: &str = "conduit.std/address-detect-kernel-hosted@1";
pub const ADDRESS_DETECT_IMPLEMENTATION: &str = "std/kernel-address-detect@1";
pub const ADDRESS_DETECT_ARTIFACT: &str = "conduit-std-host/address-detect@1";
pub const ADDRESS_DETECT_CAPABILITY: &str = "address-detect-v1";
pub const ADDRESS_DETECT_RECOGNIZED_OPERATION: &str = "conduit.host/address-detect-recognized@1";
pub const ADDRESS_DETECT_ADDRESSES_OPERATION: &str = "conduit.host/address-detect-addresses@1";

pub fn text_literal_offer() -> CapabilityOffer {
    offer(
        conduit_text::text_literal_semantics(),
        TEXT_LITERAL_CAPABILITY,
        TEXT_LITERAL_EXECUTION_PROFILE,
        TEXT_LITERAL_IMPLEMENTATION,
        TEXT_LITERAL_ARTIFACT,
        Vec::new(),
    )
}

pub fn text_upper_offer() -> CapabilityOffer {
    offer(
        conduit_text::text_upper_semantics(),
        TEXT_UPPER_CAPABILITY,
        TEXT_UPPER_EXECUTION_PROFILE,
        TEXT_UPPER_IMPLEMENTATION,
        TEXT_UPPER_ARTIFACT,
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(TEXT_UPPER_HOST_OPERATION_CONTRACT),
            target_kind: Some(kind_id(TEXT_UPPER_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
            maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
        }],
    )
}

pub fn text_join_offer() -> CapabilityOffer {
    offer(
        conduit_text::text_join_semantics(),
        TEXT_JOIN_CAPABILITY,
        TEXT_JOIN_EXECUTION_PROFILE,
        TEXT_JOIN_IMPLEMENTATION,
        TEXT_JOIN_ARTIFACT,
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(TEXT_JOIN_HOST_OPERATION_CONTRACT),
            target_kind: Some(kind_id(TEXT_JOIN_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
            maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
        }],
    )
}

pub fn text_morse_offer() -> CapabilityOffer {
    offer_semantic(
        conduit_text::text_morse_semantics().into_semantic_contract(),
        TEXT_MORSE_CAPABILITY,
        TEXT_MORSE_EXECUTION_PROFILE,
        TEXT_MORSE_IMPLEMENTATION,
        TEXT_MORSE_ARTIFACT,
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(TEXT_MORSE_HOST_OPERATION_CONTRACT),
            target_kind: Some(kind_id(TEXT_MORSE_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
            maximum_output_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        }],
    )
}

pub fn address_detect_offer() -> CapabilityOffer {
    offer(
        conduit_text::address_detect_semantics(),
        ADDRESS_DETECT_CAPABILITY,
        ADDRESS_DETECT_EXECUTION_PROFILE,
        ADDRESS_DETECT_IMPLEMENTATION,
        ADDRESS_DETECT_ARTIFACT,
        vec![
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(ADDRESS_DETECT_ADDRESSES_OPERATION),
                target_kind: Some(kind_id(conduit_text::ADDRESS_DETECT_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_text::MAX_ADDRESS_SET_VALUE_BYTES as u32,
                maximum_output_bytes: conduit_text::MAX_ADDRESS_DETECTION_VALUE_BYTES as u32,
            },
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(ADDRESS_DETECT_RECOGNIZED_OPERATION),
                target_kind: Some(kind_id(conduit_text::ADDRESS_DETECT_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
                maximum_output_bytes: conduit_text::MAX_ADDRESS_DETECTION_VALUE_BYTES as u32,
            },
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: conduit_text::TextKindContract,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    offer_semantic(
        contract.into_semantic_contract(),
        capability,
        profile,
        implementation,
        artifact,
        host_operations,
    )
}

fn offer_semantic(
    contract: Kind,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operations,
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
    fn std_offers_consume_exact_portable_text_fronts() {
        for (offer, semantic) in [
            (text_literal_offer(), conduit_text::text_literal_semantics()),
            (text_upper_offer(), conduit_text::text_upper_semantics()),
            (text_join_offer(), conduit_text::text_join_semantics()),
            (
                address_detect_offer(),
                conduit_text::address_detect_semantics(),
            ),
        ] {
            let semantic_contract = semantic.clone().into_semantic_contract();
            assert_eq!(
                offer.startup_parameters,
                semantic_contract.startup_parameters
            );
            assert_eq!(offer.shorthand, semantic_contract.shorthand);
            assert_eq!(offer.kind_id, semantic.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                semantic.kind_contract_revision
            );
            assert_eq!(offer.inputs, semantic.inputs);
            assert_eq!(offer.outputs, semantic.outputs);
            assert_eq!(offer.limits, semantic.limits);
        }
        let offer = text_morse_offer();
        let semantic = conduit_text::text_morse_semantics();
        let semantic_contract = semantic.clone().into_semantic_contract();
        assert_eq!(
            offer.startup_parameters,
            semantic_contract.startup_parameters
        );
        assert_eq!(offer.shorthand, semantic_contract.shorthand);
        assert_eq!(offer.kind_id, semantic.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            semantic.kind_contract_revision
        );
        assert_eq!(offer.inputs, semantic.inputs);
        assert_eq!(offer.outputs, semantic.outputs);
        assert_eq!(offer.limits, semantic.limits);
    }
}
