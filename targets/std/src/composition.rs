//! Explicit composition of optional operation families for the std reference host.
//!
//! A family controls which implementation code contributes offers to this host
//! image. The resulting `HostAdvertisement` remains the only runtime promise.

use crate::installed_std;
use crate::StdHostConfig;
use conduit_core::{
    HostAdvertisement, HostProfileId, PlannerCapabilityOffer, PlannerProfileId, PROTOCOL_VERSION,
};

mod resources;
mod signal;

/// Compile/composition-time selection of implementation families included in a std host image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdHostComposition {
    pub signal: bool,
    pub time: bool,
    /// Ordered nominal pulse observation; selected independently of the broad reference image.
    pub pulse_observation: bool,
    pub text: bool,
    pub input: bool,
    pub state: bool,
    pub logic: bool,
    pub math: bool,
    pub layout: bool,
    pub presentation: bool,
    pub robotics: bool,
    pub files: bool,
    pub external_websocket: bool,
    pub http: bool,
    pub json: bool,
    pub json_collection: bool,
    pub alife: bool,
}

impl StdHostComposition {
    /// The broad reference composition. This is a host image, not the definition of a host.
    pub const fn reference() -> Self {
        Self {
            signal: true,
            time: true,
            pulse_observation: false,
            text: true,
            input: true,
            state: true,
            logic: true,
            math: true,
            layout: true,
            presentation: true,
            robotics: true,
            files: true,
            external_websocket: false,
            http: true,
            json: true,
            json_collection: false,
            alife: true,
        }
    }

    /// A deliberately empty operation composition used to prove that optional families are not
    /// mandatory host-core methods.
    pub const fn minimal() -> Self {
        Self {
            signal: false,
            time: false,
            pulse_observation: false,
            text: false,
            input: false,
            state: false,
            logic: false,
            math: false,
            layout: false,
            presentation: false,
            robotics: false,
            files: false,
            external_websocket: false,
            http: false,
            json: false,
            json_collection: false,
            alife: false,
        }
    }

    pub const fn with_signal(mut self) -> Self {
        self.signal = true;
        self
    }

    pub const fn with_time(mut self) -> Self {
        self.time = true;
        self
    }

    pub const fn with_pulse_observation(mut self) -> Self {
        self.pulse_observation = true;
        self
    }

    pub const fn with_text(mut self) -> Self {
        self.text = true;
        self
    }

    pub const fn with_input(mut self) -> Self {
        self.input = true;
        self
    }

    pub const fn with_state(mut self) -> Self {
        self.state = true;
        self
    }

    pub const fn with_logic(mut self) -> Self {
        self.logic = true;
        self
    }

    pub const fn with_math(mut self) -> Self {
        self.math = true;
        self
    }

    pub const fn with_layout(mut self) -> Self {
        self.layout = true;
        self
    }

    pub const fn with_presentation(mut self) -> Self {
        self.presentation = true;
        self
    }

    pub const fn with_robotics(mut self) -> Self {
        self.robotics = true;
        self
    }

    pub const fn with_files(mut self) -> Self {
        self.files = true;
        self
    }

    pub const fn with_external_websocket(mut self) -> Self {
        self.external_websocket = true;
        self
    }

    pub const fn with_http(mut self) -> Self {
        self.http = true;
        self
    }

    pub const fn with_json_collection(mut self) -> Self {
        self.json_collection = true;
        self
    }

    pub const fn with_json(mut self) -> Self {
        self.json = true;
        self
    }

    pub const fn with_alife(mut self) -> Self {
        self.alife = true;
        self
    }
}

impl Default for StdHostComposition {
    fn default() -> Self {
        Self::reference()
    }
}

pub(super) fn build_advertisement(
    config: StdHostConfig,
    composition: StdHostComposition,
    playback: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    midi_input: Option<&crate::hosted_midi::HostedRawMidiSelection>,
    midi_output: Option<&crate::hosted_midi::MidiOutputSelection>,
    playback_proof: bool,
) -> HostAdvertisement {
    let mut capabilities = Vec::new();
    if composition.signal {
        capabilities.extend(signal::offers());
    }
    if composition.time {
        capabilities.extend([
            installed_std::tick_offer(),
            installed_std::every_offer(),
            installed_std::render_demand_offer(),
            installed_std::synth_offer(),
            conduit_std_offers::time_debounce_offer(),
            conduit_std_offers::time_timeout_offer(),
            conduit_std_offers::time_delay_offer(),
            conduit_std_offers::time_throttle_offer(),
            conduit_std_offers::recurrence_std_offer(),
            conduit_std_offers::calendar_proposal_std_offer(),
            conduit_std_offers::tick_presentation_offer(),
            conduit_std_offers::bool_presentation_offer(),
        ]);
    }
    if composition.pulse_observation {
        capabilities.push(conduit_std_offers::pulse_observe_offer());
    }
    if composition.text {
        capabilities.extend([
            conduit_std_offers::text_literal_offer(),
            conduit_std_offers::text_upper_offer(),
            conduit_std_offers::text_join_offer(),
            installed_std::text_offer(),
        ]);
    }
    if composition.input {
        capabilities.extend([
            conduit_std_offers::key_event_tee_offer(),
            conduit_std_offers::keymap_offer(),
            conduit_std_offers::chords_offer(),
            conduit_std_offers::instrument_map_std_offer(),
        ]);
    }
    if composition.state {
        capabilities.extend([
            conduit_std_offers::state_count_offer(),
            conduit_std_offers::state_toggle_offer(),
            conduit_std_offers::count_presentation_offer(),
            conduit_std_offers::state_latest_scalar_offer(),
            conduit_std_offers::flow_tee_scalar_offer(),
            conduit_std_offers::flow_gate_scalar_offer(),
            conduit_std_offers::state_select_scalar_offer(),
        ]);
    }
    if composition.logic {
        capabilities.extend([
            conduit_std_offers::logic_compare_scalar_offer(),
            conduit_std_offers::logic_not_offer(),
            conduit_std_offers::logic_select_scalar_offer(),
        ]);
    }
    if composition.math {
        capabilities.extend([
            conduit_std_offers::math_clamp_offer(),
            conduit_std_offers::math_scale_offer(),
            conduit_std_offers::math_deadband_offer(),
            conduit_std_offers::quantity_map_offer(),
            conduit_std_offers::quantity_info_offer(),
        ]);
    }
    if composition.layout {
        capabilities.extend([
            conduit_std_offers::layout_viewport_offer(),
            conduit_std_offers::layout_inset_offer(),
            conduit_std_offers::layout_row_offer(),
            conduit_std_offers::layout_column_offer(),
            conduit_std_offers::layout_stack_offer(),
            conduit_std_offers::layout_align_offer(),
        ]);
    }
    if composition.presentation {
        capabilities.extend([
            conduit_std_offers::presentation_icon_offer(),
            conduit_std_offers::presentation_frame_offer(),
            conduit_std_offers::presentation_badge_offer(),
            conduit_std_offers::graphics_rect_offer(),
            conduit_std_offers::graphics_text_offer(),
            conduit_std_offers::graphics_icon_offer(),
            conduit_std_offers::graphics_presentation_offer(),
            conduit_std_offers::bitmap_presentation_offer(),
        ]);
    }
    if composition.robotics {
        capabilities.extend([
            conduit_std_offers::robotics_observe_bump_offer(),
            conduit_std_offers::robotics_observe_imu_offer(),
            conduit_std_offers::robotics_observe_range_offer(),
            conduit_std_offers::robotics_observe_odometry_offer(),
            conduit_std_offers::robotics_observe_battery_offer(),
            conduit_std_offers::robotics_velocity_intent_offer(),
            conduit_std_offers::robotics_drive_differential_offer(),
        ]);
    }
    if composition.files {
        capabilities.extend([
            conduit_std_offers::copy_file_offer(),
            conduit_std_offers::copy_result_presentation_offer(),
        ]);
    }
    if composition.external_websocket {
        capabilities.push(conduit_net::std_external_websocket_family().capability);
    }
    if composition.http {
        capabilities.extend([
            installed_std::http_client_offer(),
            installed_std::http_server_offer(),
        ]);
    }
    if composition.json_collection {
        capabilities.push(conduit_std_offers::json_collection_step_std_offer());
    }
    if composition.json {
        capabilities.extend([
            conduit_std_offers::json_encode_std_offer(),
            conduit_std_offers::json_decode_std_offer(),
        ]);
    }
    if composition.alife {
        capabilities.extend(conduit_std_offers::alife_offers());
    }
    if playback.is_some() {
        capabilities.push(conduit_std_offers::audio_play_alsa_hw_offer());
    }
    if midi_input.is_some() {
        capabilities.push(conduit_std_offers::music_input_midi_offer());
    }
    if midi_output.is_some() {
        capabilities.push(conduit_std_offers::music_play_midi_offer());
    }
    if playback_proof {
        capabilities.push(installed_std::test_pcm_source_offer());
    }
    #[cfg(test)]
    crate::composition_test_offers::extend(&mut capabilities);
    let resources = resources::offers(composition, playback, midi_input, midi_output);

    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("rust-std"),
        resources,
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(conduit_planner::FULL_PLANNER_PROFILE),
            limits: conduit_planner::FULL_PLANNER_LIMITS,
        }],
        capabilities,
    }
}

/// Exact advertisement produced by the broad hosted reference composition.
///
/// This is Host truth: callers that need the currently installed std
/// inventory must inspect this advertisement rather than reconstructing one
/// from the portable semantic catalog.
pub fn reference_advertisement(config: StdHostConfig) -> HostAdvertisement {
    build_advertisement(
        config,
        StdHostComposition::reference(),
        None,
        None,
        None,
        false,
    )
}

/// The exact offers installed for the portable supported-nucleus contracts by
/// the broad hosted reference composition.
pub fn supported_nucleus_offers() -> Vec<conduit_core::CapabilityOffer> {
    let advertisement = reference_advertisement(StdHostConfig {
        host_id: conduit_core::HostId::from("std-inventory"),
        boot_id: conduit_core::BootId::from("std-inventory/boot"),
        offer_generation: conduit_core::OfferGeneration(1),
    });

    let contracts = conduit_semantic_catalog::supported_nucleus_contracts();
    let missing = contracts
        .iter()
        .filter(|contract| {
            !advertisement.capabilities.iter().any(|offer| {
                offer.kind_id == contract.kind_id
                    && offer.inputs == contract.inputs
                    && offer.outputs == contract.outputs
                    && offer.limits == contract.limits
            })
        })
        .map(|contract| contract.kind_id.as_str())
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "std reference composition does not install exact contracts: {missing:?}"
    );

    contracts
        .into_iter()
        .map(|contract| {
            advertisement
                .capabilities
                .iter()
                .find(|offer| {
                    offer.kind_id == contract.kind_id
                        && offer.inputs == contract.inputs
                        && offer.outputs == contract.outputs
                        && offer.limits == contract.limits
                })
                .expect("missing contracts were checked above")
                .clone()
        })
        .collect()
}

#[cfg(test)]
mod tests;
