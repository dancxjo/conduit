//! Checked public IMAGE construction; no discovery, process, PCM or device I/O.
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs,
};
use conduit_std_host::{StdHost, StdHostConfig};

pub(crate) fn selected_host(host_id: &str, boot_id: &str, speaker: bool) -> StdHost {
    let mut source = include_str!(
        "../../../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit"
    )
    .to_owned();
    if speaker {
        source=source.replace("  limits =",&format!(r#"  base = {{kind: "audio/speaker", implementation: "std/kernel-audio-play-alsa-hw@1", initialization: {{executable_sha256: "{}", card_id: "Fixture", device: 0, base_identity: "fixture/speaker"}}}}
  capability = {{kind: "sound/startup-chime", pinned_implementation: "std/kernel-startup-chime@1"}}
  capability = {{kind: "body/wake", pinned_implementation: "std/kernel-body-wake@1"}}
  need = {{id: "std/audio/alsa/fixture/speaker/card-Fixture/device-0", class: "conduit.resource/audio-playback-alsa-hw@1", slots: 1, bytes: 1}}
  limits ="#,"ab".repeat(32)));
    }
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/startup-product".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let config = StdHostConfig {
        host_id: host_id.into(),
        boot_id: boot_id.into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    if !speaker {
        return StdHost::from_selected_timer_image(config, &image, &catalog).unwrap();
    }
    use conduit_std_host::hosted_audio::{AlsaPlaybackObservation, HostedPlaybackSelection};
    let selection = HostedPlaybackSelection::from_observation(
        AlsaPlaybackObservation {
            card_index: 999,
            card_id: "Fixture".into(),
            card_name: "Fixture".into(),
            device: 0,
            device_name: "PCM".into(),
            base_identity: "fixture/speaker".into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
    )
    .with_executable_sha256("ab".repeat(32))
    .unwrap();
    StdHost::from_selected_timer_and_playback_image(config, &image, &catalog, &checked, &selection)
        .unwrap()
}
