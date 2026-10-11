//! Installed browser profile queue bounds shared by advertisement admission.
use crate::installed_browser as browser;
use conduit_core::CapabilityOffer;

pub(super) fn apply(offer: &mut CapabilityOffer, implementation: &str) {
    if matches!(
        offer.kind_id.as_str(),
        conduit_thermostat_plot::THERMOSTAT_KIND | conduit_semantic_catalog::FLOW_SCAN_KIND
    ) {
        return;
    }
    offer.limits.max_queue_bytes = browser::measurement_limits::queue_bound(offer.kind_id.as_str())
        .unwrap_or(match implementation {
            "browser/unit-dimension@1" | "browser/quantity-dimension@1" => {
                conduit_core::DIMENSION_ENCODED_LEN as u32
            }
            "browser/bounded-stroke-capture@1" => {
                browser::stroke_capture::MAXIMUM_PATH_BYTES as u32
            }
            "browser/plot-pointer-source@1"
            | "browser/select-pointer-position@1"
            | "browser/select-point-x@1"
            | "browser/select-point-y@1" => browser::NORMALIZED_POINTER_VALUE_BYTES as u32,
            _ => browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        });
}
