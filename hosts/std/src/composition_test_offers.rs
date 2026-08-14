//! Test-only capability offers kept out of production Host composition.

use crate::installed_std;
use conduit_core::CapabilityOffer;

pub(super) fn extend(capabilities: &mut Vec<CapabilityOffer>) {
    capabilities.extend([
        installed_std::test_observer_offer(),
        installed_std::test_text_source_offer(),
        installed_std::test_pcm_source_offer(),
        installed_std::test_midi_source_offer(),
        installed_std::test_key_event_source_offer(),
        installed_std::test_chord_sink_offer(),
        installed_std::test_scalar_source_offer(),
        installed_std::test_layout_sink_offer(),
        installed_std::test_presentation_sink_offer(),
        installed_std::test_graphics_sink_offer(),
        installed_std::test_scalar_literal_offer(),
        installed_std::test_scalar_sink_offer(),
        installed_std::test_gate_script_offer(),
        installed_std::test_logic_script_offer(),
        installed_std::test_logic_sink_offer(),
        installed_std::test_slow_scalar_sink_offer(),
        installed_std::test_timing_sink_offer(),
        installed_std::test_timing_source_offer(),
        installed_std::test_json_source_offer(),
        installed_std::test_json_sink_offer(),
    ]);
}
