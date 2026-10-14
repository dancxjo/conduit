//! Browser manifestation for portable pulse observations.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use conduit_core::{CapabilityOffer, PlannedGear};

const IMPLEMENTATION: &str = "browser/face-pulse@1";

fn offer() -> CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::pulse_face_contract(),
        conduit_semantic_catalog::PULSE_FACE_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/face-pulse@1",
        },
        vec![conduit_core::HostCallRequirement {
            contract_id: "conduit.host/browser-present-pulse@1".into(),
            target_kind: Some("face/browser-pulse".into()),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_time::PULSE_OBSERVATION_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![conduit_core::resource_requirement(
            conduit_core::FACE_RESOURCE_CLASS,
            1,
        )],
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserBack, String> {
    validate_placement(placement, &offer())?;
    Ok(super::BrowserBack::face(
        conduit_time::PULSE_OBSERVATION_ENCODED_LEN as u32,
        1,
    ))
}

fn perform(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    conduit_time::decode_pulse_observation(input).map_err(|error| format!("{error:?}"))?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: super::factory::manifestation_label(placement)?,
            kind_id: conduit_semantic_catalog::PULSE_FACE_KIND,
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
