//! Browser installation for the portable finite viewport source.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_kernel::{HostedValueStore, ValueStorage};
use conduit_presentation::LayoutFrame;

const IMPLEMENTATION: &str = "browser/kernel-layout-viewport@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-layout@1";

pub(super) static VIEWPORT: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::layout_viewport_contract(),
        conduit_semantic_catalog::LAYOUT_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let frame: LayoutFrame = conduit_semantic_catalog::execute_layout_source(placement)?;
    let encoded = frame.encode();
    let value = values
        .store(&encoded[..frame.encoded_len()])
        .map_err(|error| format!("store browser viewport: {error:?}"))?;
    Ok(BrowserOperation::source(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_offer_is_an_exact_local_source_without_hidden_effects() {
        let offer = offer();
        assert_eq!(
            offer.kind_id.as_str(),
            conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND
        );
        assert!(offer.inputs.is_empty());
        assert_eq!(
            offer.outputs[0].value_kind.as_str(),
            conduit_presentation::LAYOUT_FRAME_KIND
        );
        assert!(offer.host_operations.is_empty());
        assert!(offer.resource_requirements.is_empty());
    }
}
