use crate::installed_std;
use conduit_core::CapabilityOffer;
pub(super) fn append(capabilities: &mut Vec<CapabilityOffer>) {
    capabilities.extend([
        conduit_std_offers::scalar_literal_std_offer(),
        conduit_std_offers::math_clamp_offer(),
        conduit_std_offers::math_scale_offer(),
        conduit_std_offers::math_deadband_offer(),
        conduit_std_offers::quantity_map_offer(),
        conduit_std_offers::distance_frequency_map_offer(),
        conduit_std_offers::audio_time_analysis_offer(),
        conduit_std_offers::audio_spectral_analysis_offer(),
        conduit_std_offers::audio_pitch_analysis_offer(),
        conduit_std_offers::quantity_info_offer(),
    ]);
    capabilities.extend(installed_std::quantity_conversion_offers());
}
