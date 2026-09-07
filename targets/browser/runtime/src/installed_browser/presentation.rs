//! Browser presentation capability installations.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, PlannedGear,
    PRESENTATION_RESOURCE_CLASS,
};
use conduit_kernel::HostedValueStore;

const INDICATOR_IMPLEMENTATION: &str = "browser/dom-indicator@2";
const ARTIFACT: &str = "conduit-browser-runtime/installed-presentation@1";
const HOST_OPERATION: &str = "conduit.host/browser-present-indicator@1";
const BOOL_IMPLEMENTATION: &str = "browser/presentation-bool@1";
const BOOL_HOST_OPERATION: &str = "conduit.host/browser-present-current-bool@1";
const PATCHBAY_IMPLEMENTATION: &str = "browser/patchbay-surface@1";
const PATCHBAY_HOST_OPERATION: &str = "conduit.host/browser-present-patchbay@1";
const GARDEN_IMPLEMENTATION: &str = "browser/presentation-garden-state@1";
const GARDEN_HOST_OPERATION: &str = "conduit.host/browser-present-garden-state@1";

pub(super) static INDICATOR: BrowserInstallation = BrowserInstallation {
    implementation_id: INDICATOR_IMPLEMENTATION,
    offer: indicator_offer,
    prepare,
    perform: Some(perform),
};
pub(super) static BOOL: BrowserInstallation = BrowserInstallation {
    implementation_id: BOOL_IMPLEMENTATION,
    offer: bool_offer,
    prepare: prepare_bool,
    perform: Some(perform_bool),
};
pub(super) static PATCHBAY: BrowserInstallation = BrowserInstallation {
    implementation_id: PATCHBAY_IMPLEMENTATION,
    offer: patchbay_offer,
    prepare: prepare_patchbay,
    perform: Some(perform_patchbay),
};
pub(super) static GARDEN: BrowserInstallation = BrowserInstallation {
    implementation_id: GARDEN_IMPLEMENTATION,
    offer: garden_offer,
    prepare: prepare_garden,
    perform: Some(perform_garden),
};

fn garden_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::garden_state_presentation_definition();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(GARDEN_IMPLEMENTATION),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(GARDEN_IMPLEMENTATION),
            implementation_id: ImplementationId::from(GARDEN_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(GARDEN_HOST_OPERATION),
            target_kind: Some(kind_id("presentation/browser-garden-state")),
            maximum_in_flight: 1,
            maximum_input_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare_garden(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &garden_offer())?;
    Ok(BrowserOperation::presentation(
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}

fn perform_garden(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    garden_manifestation(input).map(|manifestation| BrowserHostResult {
        output: None,
        manifestation: Some(manifestation),
    })
}

fn garden_manifestation(input: &[u8]) -> Result<BrowserManifestation, String> {
    conduit_semantic_catalog::decode_garden_state(input)
        .map_err(|_| "decode Garden state presentation: malformed exact state".to_string())?;
    Ok(BrowserManifestation {
        kind_id: conduit_semantic_catalog::GARDEN_STATE_PRESENTATION_KIND,
        canonical_value: input.to_vec(),
    })
}

fn patchbay_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::patchbay_presentation_contracts()[0].clone();
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::PATCHBAY_PRESENTATION_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: PATCHBAY_IMPLEMENTATION,
            execution_profile: "browser/patchbay-kernel-hosted@1",
            implementation: PATCHBAY_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(PATCHBAY_HOST_OPERATION),
            target_kind: Some(kind_id("presentation/patchbay-surface")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_semantic_catalog::MAX_PATCHBAY_PRESENTATION_BYTES,
            maximum_output_bytes: 0,
        }],
        vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        Vec::new(),
    )
}

fn prepare_patchbay(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &patchbay_offer())?;
    Ok(BrowserOperation::presentation(
        conduit_semantic_catalog::MAX_PATCHBAY_PRESENTATION_BYTES,
        1,
    ))
}

fn perform_patchbay(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    if input.len() > conduit_semantic_catalog::MAX_PATCHBAY_PRESENTATION_BYTES as usize {
        return Err("Patchbay presentation exceeds the admitted byte bound".into());
    }
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::PATCHBAY_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn bool_offer() -> CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::bool_presentation_contract(),
        conduit_semantic_catalog::BOOL_PRESENTATION_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: BOOL_IMPLEMENTATION,
            execution_profile: BOOL_IMPLEMENTATION,
            implementation: BOOL_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(BOOL_HOST_OPERATION),
            target_kind: Some(kind_id("presentation/browser-current-bool")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        Vec::new(),
    )
}

fn prepare_bool(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &bool_offer())?;
    Ok(BrowserOperation::presentation(
        conduit_core::BOOL_ENCODED_LEN as u32,
        conduit_semantic_catalog::MAX_TOGGLE_VALUES as u32,
    ))
}

fn perform_bool(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    conduit_core::InfoBool::decode(input)
        .map_err(|error| format!("decode current Boolean presentation: {error:?}"))?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::BOOL_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn indicator_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::indicator_presentation_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("browser/indicator-presentation@2"),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_core::KindContractRevision::from(
            conduit_semantic_catalog::INDICATOR_PRESENTATION_CONTRACT_REVISION,
        ),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/presentation-indicator@2"),
            implementation_id: ImplementationId::from(INDICATOR_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(kind_id("presentation/browser-indicator")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &indicator_offer())?;
    Ok(BrowserOperation::presentation(
        placement.host_operations[0].maximum_input_bytes,
        1,
    ))
}

fn perform(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    conduit_text::MorsePattern::decode(input)
        .map_err(|error| format!("decode indicator Morse pattern: {error:?}"))?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::Scalar;

    #[test]
    fn current_bool_offer_and_manifestation_are_distinct_from_one_value_presenter() {
        let offer = bool_offer();
        assert_eq!(
            offer.kind_id.as_str(),
            conduit_semantic_catalog::BOOL_PRESENTATION_KIND
        );
        assert_eq!(
            offer.inputs[0].temporal,
            conduit_core::PortTemporal::Current
        );
        assert_eq!(
            conduit_core::InfoBool::decode(&conduit_core::InfoBool::TRUE.encode()).unwrap(),
            conduit_core::InfoBool::TRUE
        );
    }

    #[test]
    fn garden_presenter_preserves_exact_state_and_refuses_malformed_input() {
        let canonical =
            conduit_semantic_catalog::garden_state_value(conduit_semantic_catalog::GardenState {
                vitality: Scalar::from_raw_microunits(500_000),
                activity: Scalar::from_raw_microunits(250_000),
                step: 3,
            })
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let manifestation = garden_manifestation(&canonical).unwrap();
        assert_eq!(
            manifestation.kind_id,
            conduit_semantic_catalog::GARDEN_STATE_PRESENTATION_KIND
        );
        assert_eq!(manifestation.canonical_value, canonical);
        assert!(garden_manifestation(b"not Garden state").is_err());
    }
}
