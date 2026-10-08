//! Exact generic structured-value offers owned by the hosted std Host.

mod state;
pub use state::*;
mod flow_pressure;
pub use flow_pressure::*;

use conduit_core::{
    kind_id, present_host_call_requirement, resource_requirement, ArtifactId, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, ImplementationId,
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
    default_value: &conduit_core::StructuredInfoValue,
) -> CapabilityOffer {
    let contract = conduit_semantic_catalog::structured_literal_semantic_contract(
        type_name,
        value_type,
        default_value,
    )
    .expect("structured literal offer requires the exact bounded default");
    offer(contract, true, None)
}

pub fn structured_presentation_std_offer(
    type_name: &str,
    value_type: &conduit_core::StructuredInfoType,
) -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::structured_presentation_semantic_contract(type_name, value_type),
        false,
        None,
    )
}

/// Realize the canonical Quantity leaf Fore through the structured presentation Host Call.
pub fn quantity_presentation_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::quantity_presentation_semantic_contract(),
        false,
        Some(CapabilityId::from("std-quantity-presentation")),
    )
}

fn offer(
    contract: conduit_core::Kind,
    source: bool,
    capability_id: Option<CapabilityId>,
) -> CapabilityOffer {
    let value_kind = contract
        .outputs
        .first()
        .or_else(|| contract.inputs.first())
        .expect("structured contract has one runtime port")
        .value_kind
        .as_str()
        .to_string();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: capability_id.unwrap_or_else(|| {
                CapabilityId::from(format!(
                    "std-{}-{value_kind}",
                    if source {
                        "structured-literal"
                    } else {
                        "structured-presentation"
                    }
                ))
            }),
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
            host_calls: if source {
                Vec::new()
            } else {
                vec![present_host_call_requirement(
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
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_exact_portable_fronts() {
        let value_type = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
            conduit_core::BOOL_INFO_ID,
        ))
        .unwrap();
        let default_value = conduit_core::StructuredInfoValue::leaf(
            value_type.clone(),
            conduit_core::InfoBool::FALSE.encode().to_vec(),
        )
        .unwrap();
        for (offer, contract) in [
            (
                quantity_presentation_std_offer(),
                conduit_semantic_catalog::quantity_presentation_semantic_contract(),
            ),
            (
                structured_literal_std_offer("FileCopyResult", &value_type, &default_value),
                conduit_semantic_catalog::structured_literal_semantic_contract(
                    "FileCopyResult",
                    &value_type,
                    &default_value,
                )
                .unwrap(),
            ),
            (
                structured_presentation_std_offer("FileCopyResult", &value_type),
                conduit_semantic_catalog::structured_presentation_semantic_contract(
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

    #[test]
    fn quantity_and_generic_presentation_have_distinct_capability_identities() {
        let quantity = quantity_presentation_std_offer();
        let generic = structured_presentation_std_offer(
            "Quantity",
            &conduit_semantic_catalog::wrapped_quantity_type(),
        );
        assert_ne!(quantity.kind_id, generic.kind_id);
        assert_ne!(quantity.capability_id, generic.capability_id);
        assert_eq!(
            quantity.implementation.implementation_id,
            generic.implementation.implementation_id
        );
        assert_eq!(quantity.host_calls, generic.host_calls);
    }
}
