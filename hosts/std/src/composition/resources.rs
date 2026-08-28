//! Exact resource pools contributed by selected std composition families.

use super::StdHostComposition;
use crate::installed_std;
use conduit_core::{resource_offer, ResourceOffer};
use conduit_signal::signal_resource_offers;

pub(super) fn offers(
    composition: StdHostComposition,
    playback: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    midi_input: Option<&crate::hosted_midi::HostedRawMidiSelection>,
    midi_output: Option<&crate::hosted_midi::MidiOutputSelection>,
) -> Vec<ResourceOffer> {
    let mut resources = signal_resource_offers("std/timer", "std/presentation", 16);
    resources.retain(|offer| match offer.pool_id.as_str() {
        "std/timer" => composition.signal || composition.time,
        "std/presentation" => {
            composition.signal
                || composition.time
                || composition.text
                || composition.state
                || composition.logic
                || composition.math
                || composition.files
                || composition.alife
        }
        _ => false,
    });
    if composition.time {
        resources.push(resource_offer(
            "std/monotonic-deadline",
            conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
            16,
        ));
    }
    if composition.files {
        resources.push(resource_offer(
            "std/protected-file",
            conduit_std_catalog::PROTECTED_FILE_RESOURCE_CLASS,
            2,
        ));
    }
    if composition.external_websocket {
        resources.push(conduit_net::std_external_websocket_family().resource);
    }
    if composition.http {
        resources.extend([
            resource_offer(
                "std/http-client",
                installed_std::http_client_resource_class(),
                u32::from(conduit_web::HTTP_MAXIMUM_IN_FLIGHT),
            ),
            resource_offer(
                "std/http-listener",
                installed_std::http_server_resource_class(),
                1,
            ),
        ]);
    }
    if let Some(playback) = playback {
        resources.push(resource_offer(
            playback.pool_id().as_str(),
            conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS,
            1,
        ));
    }
    if let Some(midi_input) = midi_input {
        resources.push(resource_offer(
            midi_input.resource_pool_id().as_str(),
            conduit_std_offers::MIDI_INPUT_RESOURCE_CLASS,
            1,
        ));
    }
    if let Some(midi_output) = midi_output {
        resources.push(resource_offer(
            midi_output.resource_pool_id().as_str(),
            conduit_std_offers::MIDI_OUTPUT_RESOURCE_CLASS,
            1,
        ));
    }
    resources.sort();
    resources
}
