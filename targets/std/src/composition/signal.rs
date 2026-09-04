//! Exact Signal family offers for the std reference composition.

use conduit_core::CapabilityOffer;

pub(super) fn offers() -> [CapabilityOffer; 2] {
    [
        conduit_std_offers::signal_pulse_offer(),
        conduit_std_offers::signal_show_offer(),
    ]
}
