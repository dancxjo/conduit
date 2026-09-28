//! Test-only capability offers kept out of production Host composition.

use crate::installed_std;
use conduit_core::CapabilityOffer;

pub(super) fn extend(capabilities: &mut Vec<CapabilityOffer>) {
    let first = capabilities.len();
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
        installed_std::test_frequency_source_offer(),
        installed_std::test_distance_source_offer(),
        installed_std::test_tone_pcm_sink_offer(),
        installed_std::test_cancellation_source_offer(),
        installed_std::test_tone_terminal_recovery_offer(),
        installed_std::test_normal_close_sink_offer(),
        installed_std::test_gate_script_offer(),
        installed_std::test_logic_script_offer(),
        installed_std::test_logic_sink_offer(),
        installed_std::test_slow_scalar_sink_offer(),
        installed_std::test_timing_sink_offer(),
        installed_std::test_timing_source_offer(),
        installed_std::test_timing_unit_source_offer(),
        installed_std::test_json_source_offer(),
        installed_std::test_json_sink_offer(),
        conduit_std_offers::deterministic_speech_offer(),
        conduit_std_offers::deterministic_streaming_speech_offer(),
        installed_std::test_speech_sink_offer(),
    ]);
    capabilities.extend(installed_std::test_data_terminal_recovery_offers());
    capabilities.push(installed_std::test_data_text_sink_offer());
    capabilities.push(installed_std::test_data_text_source_offer());
    let catalog = installed_std::test_catalog();
    for offer in &mut capabilities[first..] {
        if let Some(kind) = catalog.canonical_kind(&offer.kind_id) {
            offer.semantic_contract = kind.semantic_contract();
        } else if let Some(definition) = catalog.get(&offer.kind_id) {
            offer.semantic_contract = conduit_core::KindSemanticContract {
                configuration: definition.configuration.clone(),
                laws: Vec::new(),
            };
        }
    }
}
