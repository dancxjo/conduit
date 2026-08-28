//! Exact artificial-life realizations owned by the hosted std Host.

use conduit_alife::{
    LENIA_MAXIMUM_FIELD_BYTES, LENIA_STEP_KIND, LENIA_STEP_REVISION, ORBIUM_SEED_REVISION,
    SCALAR_FIELD_PRESENTATION_REVISION,
};
use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement, CapabilityOffer,
    HostOperationContractId, HostOperationRequirement, PRESENTATION_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub const ORBIUM_SEED_EXECUTION_PROFILE: &str = "conduit.std/orbium-seed-fixed-q16.16@1";
pub const LENIA_STEP_EXECUTION_PROFILE: &str = "conduit.std/lenia-spatial-fixed-q16.16@1";
pub const SCALAR_FIELD_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/present-scalar-field-terminal@1";
pub const ORBIUM_SEED_IMPLEMENTATION: &str = "std/kernel-orbium-seed@1";
pub const LENIA_STEP_IMPLEMENTATION: &str = "std/kernel-lenia-step@1";
pub const SCALAR_FIELD_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-present-scalar-field@1";
pub const ORBIUM_SEED_ARTIFACT: &str = "conduit-std-host/orbium-seed@1";
pub const LENIA_STEP_ARTIFACT: &str = "conduit-std-host/lenia-spatial-q16@1";
pub const SCALAR_FIELD_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-scalar-field@1";
pub const LENIA_INITIALIZE_HOST_OPERATION: &str = "conduit.host/lenia-initialize@1";
pub const LENIA_STEP_HOST_OPERATION: &str = "conduit.host/lenia-step@1";
pub const SCALAR_FIELD_PRESENTATION_TARGET: &str = "presentation/stdout-scalar-field";

pub fn alife_offers() -> Vec<CapabilityOffer> {
    vec![
        orbium_seed_offer(),
        lenia_step_offer(),
        scalar_field_presentation_offer(),
    ]
}

pub fn orbium_seed_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::orbium_seed_contract(),
        ORBIUM_SEED_REVISION,
        "std-orbium-seed-v1",
        ORBIUM_SEED_EXECUTION_PROFILE,
        ORBIUM_SEED_IMPLEMENTATION,
        ORBIUM_SEED_ARTIFACT,
        Vec::new(),
        Vec::new(),
    )
}

pub fn lenia_step_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::lenia_step_contract(),
        LENIA_STEP_REVISION,
        "std-lenia-step-v1",
        LENIA_STEP_EXECUTION_PROFILE,
        LENIA_STEP_IMPLEMENTATION,
        LENIA_STEP_ARTIFACT,
        vec![
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(LENIA_INITIALIZE_HOST_OPERATION),
                target_kind: Some(kind_id(LENIA_STEP_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: LENIA_MAXIMUM_FIELD_BYTES,
                maximum_output_bytes: 0,
            },
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(LENIA_STEP_HOST_OPERATION),
                target_kind: Some(kind_id(LENIA_STEP_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_time::TICK_ENCODED_LEN,
                maximum_output_bytes: LENIA_MAXIMUM_FIELD_BYTES,
            },
        ],
        Vec::new(),
    )
}

pub fn scalar_field_presentation_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::scalar_field_presentation_contract(),
        SCALAR_FIELD_PRESENTATION_REVISION,
        "std-scalar-field-presentation-v1",
        SCALAR_FIELD_PRESENTATION_EXECUTION_PROFILE,
        SCALAR_FIELD_PRESENTATION_IMPLEMENTATION,
        SCALAR_FIELD_PRESENTATION_ARTIFACT,
        vec![present_host_operation_requirement(
            kind_id(SCALAR_FIELD_PRESENTATION_TARGET),
            LENIA_MAXIMUM_FIELD_BYTES,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
    )
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
    host_operations: Vec<HostOperationRequirement>,
    resources: Vec<conduit_core::ResourceRequirement>,
) -> CapabilityOffer {
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability,
            execution_profile,
            implementation,
            artifact,
        },
        host_operations,
        resources,
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offer_preserves_its_portable_contract() {
        for (offer, contract) in [
            (
                orbium_seed_offer(),
                conduit_semantic_catalog::orbium_seed_contract(),
            ),
            (
                lenia_step_offer(),
                conduit_semantic_catalog::lenia_step_contract(),
            ),
            (
                scalar_field_presentation_offer(),
                conduit_semantic_catalog::scalar_field_presentation_contract(),
            ),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
    }
}
