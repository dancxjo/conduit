//! Exact typed tick manifestation through the ordinary presentation operation.
use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{CapabilityOffer, PlannedGear};

const IMPLEMENTATION: &str = "browser/presentation-tick@1";
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: Some(perform),
};

fn offer() -> CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::tick_presentation_contract(),
        conduit_semantic_catalog::TICK_PRESENTATION_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/installed-tick@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: "conduit.host/browser-present-tick@1".into(),
            target_kind: Some("presentation/browser-tick".into()),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_time::TICK_ENCODED_LEN,
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
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserOperation::presentation(
        conduit_time::TICK_ENCODED_LEN,
        1,
    ))
}

fn perform(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    conduit_time::decode_tick(input).map_err(|error| error.to_string())?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::TICK_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}
