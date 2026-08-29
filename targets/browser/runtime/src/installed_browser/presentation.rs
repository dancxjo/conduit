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

pub(super) static INDICATOR: BrowserInstallation = BrowserInstallation {
    implementation_id: INDICATOR_IMPLEMENTATION,
    offer: indicator_offer,
    prepare,
    perform: Some(perform),
};

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
