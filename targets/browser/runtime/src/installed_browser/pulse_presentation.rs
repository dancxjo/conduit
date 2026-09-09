//! Browser manifestation for portable pulse observations.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use conduit_core::{CapabilityOffer, PlannedGear};

const IMPLEMENTATION: &str = "browser/presentation-pulse@1";

fn offer() -> CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::pulse_presentation_contract(),
        conduit_semantic_catalog::PULSE_PRESENTATION_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/presentation-pulse@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: "conduit.host/browser-present-pulse@1".into(),
            target_kind: Some("presentation/browser-pulse".into()),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_time::PULSE_OBSERVATION_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![conduit_core::resource_requirement(
            conduit_core::PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    Ok(super::BrowserOperation::presentation(
        conduit_time::PULSE_OBSERVATION_ENCODED_LEN as u32,
        1,
    ))
}

fn perform(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    conduit_time::decode_pulse_observation(input).map_err(|error| format!("{error:?}"))?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::PULSE_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: Some(perform),
};
