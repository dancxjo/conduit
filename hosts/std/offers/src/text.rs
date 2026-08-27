//! Exact text realization offers owned by the hosted std Host.

use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
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

pub fn text_literal_offer() -> CapabilityOffer {
    offer(
        conduit_text::text_literal_semantics(),
        TEXT_LITERAL_CAPABILITY,
        TEXT_LITERAL_EXECUTION_PROFILE,
        TEXT_LITERAL_IMPLEMENTATION,
        TEXT_LITERAL_ARTIFACT,
        vec![FaceStartupParameter {
            name: "value".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        None,
    )
}

pub fn text_upper_offer() -> CapabilityOffer {
    let mut offer = offer(
        conduit_text::text_upper_semantics(),
        TEXT_UPPER_CAPABILITY,
        TEXT_UPPER_EXECUTION_PROFILE,
        TEXT_UPPER_IMPLEMENTATION,
        TEXT_UPPER_ARTIFACT,
        Vec::new(),
        Some((port_id("text"), port_id("text"))),
    );
    offer.host_operations.push(HostOperationRequirement {
        contract_id: HostOperationContractId::from(TEXT_UPPER_HOST_OPERATION_CONTRACT),
        target_kind: Some(kind_id(TEXT_UPPER_HOST_OPERATION_TARGET)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
    });
    offer
}

pub fn text_join_offer() -> CapabilityOffer {
    let mut offer = offer(
        conduit_text::text_join_semantics(),
        TEXT_JOIN_CAPABILITY,
        TEXT_JOIN_EXECUTION_PROFILE,
        TEXT_JOIN_IMPLEMENTATION,
        TEXT_JOIN_ARTIFACT,
        vec![FaceStartupParameter {
            name: "prefix".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        Some((port_id("text"), port_id("text"))),
    );
    offer.host_operations.push(HostOperationRequirement {
        contract_id: HostOperationContractId::from(TEXT_JOIN_HOST_OPERATION_CONTRACT),
        target_kind: Some(kind_id(TEXT_JOIN_HOST_OPERATION_TARGET)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
    });
    offer
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: conduit_text::TextKindContract,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    startup_parameters: Vec<FaceStartupParameter>,
    shorthand: Option<(conduit_core::PortId, conduit_core::PortId)>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters,
        shorthand,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_offers_consume_exact_portable_text_faces() {
        for (offer, semantic) in [
            (text_literal_offer(), conduit_text::text_literal_semantics()),
            (text_upper_offer(), conduit_text::text_upper_semantics()),
            (text_join_offer(), conduit_text::text_join_semantics()),
        ] {
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
}
