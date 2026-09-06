//! Exact generic structured-value offers owned by the hosted std Host.

mod state;
pub use state::*;

use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement, ArtifactId, CapabilityId,
    CapabilityOffer, ExecutionProfileId, ImplementationId, ImplementationOffer,
    PRESENTATION_RESOURCE_CLASS,
};

pub const STRUCTURED_LITERAL_STD_PROFILE: &str = "std/structured-literal-kernel@1";
pub const STRUCTURED_PRESENTATION_STD_PROFILE: &str = "std/structured-presentation-kernel@1";
pub const STRUCTURED_LITERAL_STD_IMPLEMENTATION: &str = "std/kernel-structured-literal@1";
pub const STRUCTURED_PRESENTATION_STD_IMPLEMENTATION: &str = "std/kernel-structured-presentation@1";
pub const STRUCTURED_LITERAL_STD_ARTIFACT: &str = "conduit-core/structured-info@1";
pub const STRUCTURED_PRESENTATION_STD_ARTIFACT: &str = "conduit-presentation/structured-info@1";

pub fn structured_literal_std_offer(
    type_name: &str,
    value_type: &conduit_core::StructuredInfoType,
) -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::structured_literal_contract(type_name, value_type),
        true,
    )
}

pub fn structured_presentation_std_offer(
    type_name: &str,
    value_type: &conduit_core::StructuredInfoType,
) -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::structured_presentation_contract(type_name, value_type),
        false,
    )
}

fn offer(
    contract: conduit_semantic_catalog::StructuredValueContract,
    source: bool,
) -> CapabilityOffer {
    let value_kind = contract
        .outputs
        .first()
        .or_else(|| contract.inputs.first())
        .expect("structured contract has one runtime port")
        .value_kind
        .as_str();
    CapabilityOffer {
        startup_parameters: contract.startup_parameters,
        shorthand: None,
        capability_id: CapabilityId::from(format!(
            "std-{}-{value_kind}",
            if source {
                "structured-literal"
            } else {
                "structured-presentation"
            }
        )),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(if source {
                STRUCTURED_LITERAL_STD_PROFILE
            } else {
                STRUCTURED_PRESENTATION_STD_PROFILE
            }),
            implementation_id: ImplementationId::from(if source {
                STRUCTURED_LITERAL_STD_IMPLEMENTATION
            } else {
                STRUCTURED_PRESENTATION_STD_IMPLEMENTATION
            }),
            artifact_id: ArtifactId::from(if source {
                STRUCTURED_LITERAL_STD_ARTIFACT
            } else {
                STRUCTURED_PRESENTATION_STD_ARTIFACT
            }),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: if source {
            Vec::new()
        } else {
            vec![present_host_operation_requirement(
                kind_id(conduit_semantic_catalog::STRUCTURED_PRESENTATION_TARGET),
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            )]
        },
        resource_requirements: if source {
            Vec::new()
        } else {
            vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
        },
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_exact_portable_faces() {
        let value_type = conduit_semantic_catalog::copy_result_type();
        for (offer, contract) in [
            (
                structured_literal_std_offer("FileCopyResult", &value_type),
                conduit_semantic_catalog::structured_literal_contract(
                    "FileCopyResult",
                    &value_type,
                ),
            ),
            (
                structured_presentation_std_offer("FileCopyResult", &value_type),
                conduit_semantic_catalog::structured_presentation_contract(
                    "FileCopyResult",
                    &value_type,
                ),
            ),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                contract.kind_contract_revision
            );
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
    }
}
