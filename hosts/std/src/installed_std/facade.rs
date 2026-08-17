//! Narrow installed-std offer and proof-catalog facade.

pub(crate) use super::contract::text_offer;
pub(crate) use super::http::{
    client_offer as http_client_offer, server_offer as http_server_offer,
};
pub(crate) use super::render_demand_operation::offer as render_demand_offer;
pub(crate) use super::synth_operation::offer as synth_offer;
#[cfg(test)]
pub(crate) use super::test_json_codec::{
    sink_offer as test_json_sink_offer, source_offer as test_json_source_offer,
};
#[cfg(test)]
pub(crate) use super::test_support::{
    test_catalog, test_graphics_sink_offer, test_layout_sink_offer, test_observer_offer,
    test_presentation_sink_offer,
};

pub(crate) fn http_client_resource_class() -> &'static str {
    super::http::CLIENT_RESOURCE
}

pub(crate) fn http_server_resource_class() -> &'static str {
    super::http::SERVER_RESOURCE
}

#[cfg(test)]
pub(crate) fn test_text_source_offer() -> conduit_core::CapabilityOffer {
    super::test_text_source::offer()
}

pub(crate) fn test_pcm_source_offer() -> conduit_core::CapabilityOffer {
    super::test_audio_source::offer()
}

#[cfg(test)]
pub(crate) fn test_midi_source_offer() -> conduit_core::CapabilityOffer {
    super::test_midi_source::offer()
}

pub(crate) fn playback_proof_catalog() -> conduit_form::ProfileCatalog {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_sound_catalogs(&mut startup, &mut profile)
        .expect("sound proof catalog identities are unique");
    super::test_audio_source::install_catalog(&mut profile);
    profile
}

#[cfg(test)]
pub(crate) fn test_key_event_source_offer() -> conduit_core::CapabilityOffer {
    super::test_input_semantics::source_offer()
}

#[cfg(test)]
pub(crate) fn test_chord_sink_offer() -> conduit_core::CapabilityOffer {
    super::test_input_semantics::sink_offer()
}

#[cfg(test)]
pub(crate) fn test_scalar_source_offer() -> conduit_core::CapabilityOffer {
    super::test_scalar_flow::source_offer()
}

#[cfg(test)]
pub(crate) fn test_scalar_literal_offer() -> conduit_core::CapabilityOffer {
    super::test_scalar_flow::literal_offer()
}

#[cfg(test)]
pub(crate) fn test_scalar_sink_offer() -> conduit_core::CapabilityOffer {
    super::test_scalar_flow::sink_offer()
}

#[cfg(test)]
pub(crate) fn test_gate_script_offer() -> conduit_core::CapabilityOffer {
    super::test_gate::source_offer()
}

#[cfg(test)]
pub(crate) fn test_logic_script_offer() -> conduit_core::CapabilityOffer {
    super::test_logic::offer()
}

#[cfg(test)]
pub(crate) fn test_logic_sink_offer() -> conduit_core::CapabilityOffer {
    super::test_logic::sink_offer()
}

#[cfg(test)]
pub(crate) fn test_slow_scalar_sink_offer() -> conduit_core::CapabilityOffer {
    super::test_gate::slow_sink_offer()
}

#[cfg(test)]
pub(crate) fn test_timing_sink_offer() -> conduit_core::CapabilityOffer {
    super::test_timing_sink::offer()
}

#[cfg(test)]
pub(crate) fn test_timing_source_offer() -> conduit_core::CapabilityOffer {
    super::test_timing_sink::source_offer()
}
