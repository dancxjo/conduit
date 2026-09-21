//! Browser installations for the portable application event/state/view seam.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{resource_requirement, CapabilityOffer, HostCallRequirement, PlannedGear};
use conduit_kernel::HostedValueStore;

pub(crate) const EVENT_IMPLEMENTATION: &str = "browser/application-event-source@1";
pub(crate) const STATE_IMPLEMENTATION: &str = "browser/retained-application@1";
pub(crate) const PRESENTATION_IMPLEMENTATION: &str = "browser/application-view-presentation@1";
pub(crate) const EVENT_OPERATION: &str = "conduit.host/browser-application-next-event@1";
pub(crate) const STATE_OPERATION: &str = "conduit.host/browser-application-apply-event@1";
pub(crate) const PRESENTATION_OPERATION: &str = "conduit.host/browser-present-application-view@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-application@1";
pub(crate) const EVENT_BYTES: u32 = 4096;
const VIEW_BYTES: u32 = 16 * 1024;

pub(super) static EVENT: BrowserInstallation = BrowserInstallation {
    implementation_id: EVENT_IMPLEMENTATION,
    offer: event_offer,
    prepare: prepare_event,
    perform: None,
};
pub(super) static STATE: BrowserInstallation = BrowserInstallation {
    implementation_id: STATE_IMPLEMENTATION,
    offer: state_offer,
    prepare: prepare_state,
    perform: None,
};
pub(super) static PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: PRESENTATION_IMPLEMENTATION,
    offer: presentation_offer,
    prepare: prepare_presentation,
    perform: Some(perform_presentation),
};

fn event_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::event_source_contract(),
        EVENT_IMPLEMENTATION,
        EVENT_OPERATION,
        0,
        EVENT_BYTES,
        false,
    )
}
fn state_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::retained_application_contract(),
        STATE_IMPLEMENTATION,
        STATE_OPERATION,
        EVENT_BYTES,
        VIEW_BYTES,
        false,
    )
}
fn presentation_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::view_presentation_contract(),
        PRESENTATION_IMPLEMENTATION,
        PRESENTATION_OPERATION,
        VIEW_BYTES,
        0,
        true,
    )
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    implementation: &'static str,
    operation: &'static str,
    input: u32,
    output: u32,
    presentation: bool,
) -> CapabilityOffer {
    let mut result = conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::APPLICATION_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: "browser/bounded-application@1",
            implementation,
            artifact: ARTIFACT,
        },
        vec![HostCallRequirement {
            contract_id: operation.into(),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: input,
            maximum_output_bytes: output,
        }],
        if presentation {
            vec![resource_requirement(
                conduit_core::PRESENTATION_RESOURCE_CLASS,
                1,
            )]
        } else {
            vec![]
        },
        vec![],
    );
    result.limits.max_queue_items = 1;
    result.limits.max_queue_bytes = input.max(output);
    result
}

fn prepare_event(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &event_offer())?;
    BrowserOperation::host_source(values, EVENT_BYTES)
}

fn prepare_state(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &state_offer())?;
    BrowserOperation::application_state(values, EVENT_BYTES)
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &presentation_offer())?;
    Ok(BrowserOperation::presentation(VIEW_BYTES, 1))
}

fn perform_presentation(
    _placement: &PlannedGear,
    input: &[u8],
) -> Result<BrowserHostResult, String> {
    conduit_presentation::ApplicationView::decode(input)
        .map_err(|error| format!("decode application view: {error:?}"))?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::APPLICATION_VIEW_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

pub(crate) fn application_id(placement: &PlannedGear) -> Result<&str, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (
                conduit_semantic_catalog::APPLICATION_ID_CONFIGURATION,
                conduit_core::ConfigurationValue::Text(value),
            ) => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(|| "retained application has no exact application identity".into())
}
