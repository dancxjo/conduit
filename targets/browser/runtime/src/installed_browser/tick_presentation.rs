//! Exact typed Tick manifestation through the existing browser presentation operation.
use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{CapabilityOffer, ConfigurationValue, HostOperationRequirement, PlannedGear};
use conduit_kernel::HostedValueStore;

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
            artifact: "conduit-browser-runtime/tick-presentation@1",
        },
        vec![HostOperationRequirement {
            contract_id: "conduit.host/browser-present-tick@1".into(),
            target_kind: Some(conduit_semantic_catalog::TICK_PRESENTATION_KIND.into()),
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

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-values", ConfigurationValue::U64(value))
                if (1..=conduit_time::TIME_EVERY_COUNT).contains(value) =>
            {
                Some(*value as u32)
            }
            _ => None,
        })
        .ok_or("tick presentation has no admitted maximum")?;
    Ok(BrowserOperation::presentation(
        conduit_time::TICK_ENCODED_LEN,
        maximum,
    ))
}

fn perform(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    conduit_time::decode_tick(input)
        .map_err(|error| format!("invalid tick manifestation: {error:?}"))?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::TICK_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}
