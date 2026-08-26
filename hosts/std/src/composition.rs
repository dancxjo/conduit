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
    pub alife: bool,
}

impl StdHostComposition {
    /// The broad reference composition. This is a host image, not the definition of a host.
    pub const fn reference() -> Self {
        Self {
            signal: true,
            time: true,
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
            alife: true,
        }
    }

    /// A deliberately empty operation composition used to prove that optional families are not
    /// mandatory host-core methods.
    pub const fn minimal() -> Self {
        Self {
            signal: false,
            time: false,
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
            conduit_std_catalog::time_debounce_offer(),
            conduit_std_catalog::time_timeout_offer(),
            conduit_std_catalog::time_delay_offer(),
            conduit_std_catalog::time_throttle_offer(),
            conduit_std_catalog::recurrence_std_offer(),
            conduit_std_catalog::calendar_proposal_std_offer(),
            conduit_std_catalog::tick_presentation_offer(),
            conduit_std_catalog::bool_presentation_std_offer(),
        ]);
    }
    if composition.text {
        capabilities.extend([
            conduit_std_catalog::text_literal_offer(),
            conduit_std_catalog::text_upper_offer(),
            conduit_std_catalog::text_join_offer(),
            installed_std::text_offer(),
        ]);
    }
    if composition.input {
        capabilities.extend([
            conduit_std_catalog::key_event_tee_offer(),
            conduit_std_catalog::keymap_offer(),
            conduit_std_catalog::chords_offer(),
            conduit_std_catalog::instrument_map_std_offer(),
        ]);
    }
    if composition.state {
        capabilities.extend([
            conduit_std_catalog::state_count_offer(),
            conduit_std_catalog::state_toggle_offer(),
            conduit_std_catalog::count_presentation_offer(),
            conduit_std_catalog::state_latest_scalar_offer(),
            conduit_std_catalog::flow_tee_scalar_offer(),
            conduit_std_catalog::flow_gate_scalar_offer(),
            conduit_std_catalog::state_select_scalar_offer(),
        ]);
    }
    if composition.logic {
        capabilities.extend([
            conduit_std_catalog::logic_compare_scalar_offer(),
            conduit_std_catalog::logic_not_offer(),
            conduit_std_catalog::logic_select_scalar_offer(),
        ]);
    }
    if composition.math {
        capabilities.extend([
            conduit_std_catalog::math_clamp_offer(),
            conduit_std_catalog::math_scale_offer(),
            conduit_std_catalog::math_deadband_offer(),
        ]);
    }
    if composition.layout {
        capabilities.extend([
            conduit_std_catalog::layout_viewport_offer(),
            conduit_std_catalog::layout_inset_offer(),
            conduit_std_catalog::layout_row_offer(),
            conduit_std_catalog::layout_column_offer(),
            conduit_std_catalog::layout_stack_offer(),
            conduit_std_catalog::layout_align_offer(),
        ]);
    }
    if composition.presentation {
        capabilities.extend([
            conduit_std_catalog::presentation_icon_offer(),
            conduit_std_catalog::presentation_frame_offer(),
            conduit_std_catalog::presentation_badge_offer(),
            conduit_std_catalog::graphics_rect_offer(),
            conduit_std_catalog::graphics_text_offer(),
            conduit_std_catalog::graphics_icon_offer(),
            conduit_std_catalog::graphics_presentation_offer(),
            conduit_std_catalog::bitmap_presentation_offer(),
        ]);
    }
    if composition.robotics {
        capabilities.extend([
            conduit_std_catalog::robotics_observe_bump_offer(),
            conduit_std_catalog::robotics_observe_imu_offer(),
            conduit_std_catalog::robotics_observe_range_offer(),
            conduit_std_catalog::robotics_observe_odometry_offer(),
            conduit_std_catalog::robotics_observe_battery_offer(),
            conduit_std_catalog::robotics_velocity_intent_offer(),
            conduit_std_catalog::robotics_drive_differential_offer(),
        ]);
    }
    if composition.files {
        capabilities.extend([
            conduit_std_catalog::copy_file_offer(),
            conduit_std_catalog::copy_result_presentation_offer(),
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
    if composition.json {
        capabilities.extend([
            conduit_std_catalog::json_encode_std_offer(),
            conduit_std_catalog::json_decode_std_offer(),
        ]);
    }
    if composition.alife {
        capabilities.extend(conduit_std_catalog::alife_offers());
    }
    if playback.is_some() {
        capabilities.push(conduit_std_catalog::audio_play_alsa_hw_offer());
    }
    if midi_input.is_some() {
        capabilities.push(conduit_std_catalog::music_input_midi_offer());
    }
    if midi_output.is_some() {
        capabilities.push(conduit_std_catalog::music_play_midi_offer());
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

#[cfg(test)]
mod tests {
    use super::StdHostComposition;
    use crate::{StdHost, StdHostConfig};
    use conduit_core::{BootId, HostId, OfferGeneration};

    fn host(composition: StdHostComposition) -> StdHost {
        StdHost::new_with_composition(
            StdHostConfig {
                host_id: HostId::from("composition-test"),
                boot_id: BootId::from("composition-boot"),
                offer_generation: OfferGeneration(1),
            },
            composition,
        )
    }

    fn offered(host: &StdHost, kind: &str) -> bool {
        host.advertisement()
            .capabilities
            .iter()
            .any(|offer| offer.kind_id.as_str() == kind)
    }

    #[test]
    fn a_selected_family_contributes_only_its_exact_operation_offers() {
        let host = host(StdHostComposition::minimal().with_text());

        assert!(offered(&host, "text/literal"));
        assert!(offered(&host, "text/upper"));
        assert!(offered(&host, "text/join"));
        assert!(offered(&host, "presentation/text"));
        assert!(!offered(&host, "flow/pulse"));
        assert!(!offered(&host, "time/every"));
        assert!(!offered(&host, "state/count"));
        assert_eq!(host.advertisement().resources.len(), 1);
        assert_eq!(
            host.advertisement().resources[0].pool_id.as_str(),
            "std/presentation"
        );
    }

    #[test]
    fn external_websocket_listener_is_an_explicit_capability_and_resource_family() {
        let omitted = host(StdHostComposition::minimal());
        assert!(!offered(
            &omitted,
            conduit_net::EXTERNAL_WEBSOCKET_LISTENER_KIND
        ));
        assert!(omitted.advertisement().resources.is_empty());

        let selected = host(StdHostComposition::minimal().with_external_websocket());
        assert!(offered(
            &selected,
            conduit_net::EXTERNAL_WEBSOCKET_LISTENER_KIND
        ));
        assert_eq!(selected.advertisement().resources.len(), 1);
        assert_eq!(
            selected.advertisement().resources[0].class_id.as_str(),
            conduit_net::EXTERNAL_WEBSOCKET_LISTENER_RESOURCE
        );
    }

    #[test]
    fn hosted_http_is_opt_in_and_seals_resources_operations_and_authority() {
        let omitted = host(StdHostComposition::minimal());
        assert!(!offered(&omitted, conduit_web::HTTP_CLIENT_KIND));
        assert!(!offered(&omitted, conduit_web::HTTP_SERVER_KIND));

        let selected = host(StdHostComposition::minimal().with_http());
        let client = selected
            .advertisement()
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND)
            .unwrap();
        let server = selected
            .advertisement()
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == conduit_web::HTTP_SERVER_KIND)
            .unwrap();
        assert_eq!(client.host_operations.len(), 1);
        assert_eq!(client.authority_requirements.len(), 1);
        assert_eq!(server.host_operations.len(), 2);
        assert_eq!(server.authority_requirements.len(), 2);
        assert_eq!(selected.advertisement().resources.len(), 2);
    }

    #[test]
    fn compiled_families_are_not_ambient_runtime_promises() {
        let minimal = host(StdHostComposition::minimal());
        let reference = host(StdHostComposition::reference());

        for kind in [
            "flow/pulse",
            "presentation/show",
            "time/tick",
            "time/every",
            "time/debounce",
            "time/timeout",
            "time/delay",
            "time/throttle",
            "text/literal",
            "text/upper",
            "text/join",
            "presentation/text",
            "state/count",
            "state/toggle",
            "presentation/count",
            "state/latest",
            "flow/tee",
            "flow/gate",
            "state/select",
            "robotics/observe-bump",
            "robotics/observe-imu",
            "robotics/observe-range",
            "robotics/observe-odometry",
            "robotics/observe-battery",
            "robotics/velocity-intent",
            "robotics/drive-differential",
            "file/copy",
            conduit_web::HTTP_CLIENT_KIND,
            conduit_web::HTTP_SERVER_KIND,
        ] {
            assert!(!offered(&minimal, kind), "minimal host offered {kind}");
            assert!(offered(&reference, kind), "reference host omitted {kind}");
        }
        assert!(minimal.advertisement().resources.is_empty());
    }

    #[test]
    fn planner_cannot_obtain_an_unselected_family_from_a_category_prefix() {
        let host = host(StdHostComposition::minimal().with_text());
        let form = conduit_form::parse_with_startup(
            include_str!("../../../fixtures/forms/signal-demo.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &conduit_signal::signal_profile_catalog(),
        )
        .expect("Signal form checks independently of host composition");

        assert!(host.plan_local(&form, None).is_err());
    }

    #[test]
    fn reference_host_browser_and_pico_have_different_exact_offer_sets() {
        let std = host(StdHostComposition::reference());
        let browser = conduit_signal_conformance::distributed_browser_sink_advertisement();
        let pico = conduit_signal_conformance::pico_local_advertisement();

        let kinds = |advertisement: &conduit_core::HostAdvertisement| {
            advertisement
                .capabilities
                .iter()
                .map(|offer| offer.kind_id.as_str().to_owned())
                .collect::<std::collections::BTreeSet<_>>()
        };

        assert_ne!(kinds(std.advertisement()), kinds(&browser));
        assert_ne!(kinds(std.advertisement()), kinds(&pico));
        assert_ne!(kinds(&browser), kinds(&pico));
    }

    #[test]
    fn reference_host_advertises_every_supported_std_revision_and_no_legacy_revision() {
        let host = host(StdHostComposition::reference());
        let advertised = host
            .advertisement()
            .capabilities
            .iter()
            .filter(|offer| {
                let revision = offer.kind_contract_revision.as_str();
                offer.kind_id.as_str() != conduit_std_catalog::INSTRUMENT_MAP_KIND
                    && (revision.starts_with("conduit.std/")
                        || revision.starts_with("conduit.input/")
                        || offer.kind_id.as_str() == conduit_std_catalog::BOOL_PRESENTATION_KIND
                        || offer.kind_id.as_str() == conduit_std_catalog::BITMAP_PRESENTATION_KIND)
            })
            .cloned()
            .collect::<Vec<_>>();
        let supported = conduit_std_catalog::supported_nucleus_offers()
            .into_iter()
            .filter(|offer| {
                offer
                    .implementation
                    .implementation_id
                    .as_str()
                    .starts_with("std/")
            })
            .collect::<Vec<_>>();

        assert_eq!(advertised, supported);
        assert!(host
            .advertisement()
            .capabilities
            .iter()
            .any(|offer| { offer == &conduit_std_catalog::instrument_map_std_offer() }));
    }
}
