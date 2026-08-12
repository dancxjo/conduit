use crate::hosted_audio::{
    AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection, PlaybackLifecycle,
};
use crate::hosted_midi::{
    HostedRawMidiSelection, MidiEndpointDirection, MidiInputLifecycle, RawMidiEndpointObservation,
};
use crate::{
    RunControl, RunControlRequestId, StdHost, StdHostComposition, StdHostConfig, TimerAdapter,
};
use conduit_core::{BootId, ConnectionBase, HostId, OfferGeneration, TerminalDisposition};
use std::collections::BTreeMap;
use std::time::Duration;

fn host() -> StdHost {
    let config = StdHostConfig {
        host_id: HostId::from("instrument-fixture-host"),
        boot_id: BootId::from("instrument-fixture-boot"),
        offer_generation: OfferGeneration(19),
    };
    let input = HostedRawMidiSelection::select(
        &[RawMidiEndpointObservation {
            card: u16::MAX,
            device: u16::MAX,
            subdevice: 0,
            name: "Bounded instrument input".into(),
            direction: MidiEndpointDirection::ReadableSource,
        }],
        MidiEndpointDirection::ReadableSource,
        u16::MAX,
        u16::MAX,
        0,
        config.boot_id.clone(),
        config.offer_generation,
    )
    .unwrap()
    .with_fake_input(vec![0x90, 60, 100]);
    let playback = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 0,
            card_id: "FIXTURE".into(),
            card_name: "Instrument playback fixture".into(),
            device: 0,
            device_name: "Finite PCM sink".into(),
            base_identity: "instrument-playback-base".into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
        FakePlaybackBehavior::Success,
    );
    StdHost::new_with_raw_midi_input_and_playback(
        config,
        StdHostComposition::reference(),
        input,
        playback,
    )
    .unwrap()
}

fn fragment(host: &StdHost) -> conduit_core::PlanFragment {
    plan_fragment(host, true).unwrap()
}

fn plan_fragment(
    host: &StdHost,
    observe_render_clock: bool,
) -> Result<conduit_core::PlanFragment, conduit_planner::PlannerError> {
    let form = conduit_form::parse(
        "form 0\n\ninstrument {\n input: music/input\n render: audio/render-demand\n synth: music/synth\n output: audio/play\n input.a4-reference-millihertz = 442000\n input.transpose-semitones = 12\n synth.maximum-voices = 8\n synth.oscillator = \"saw\"\n input.notes -> synth.notes\n input.controls -> synth.controls\n render.demand -> synth.render\n synth.audio -> output.audio\n}\n",
        &crate::installed_std::test_catalog(),
    )
    .unwrap();
    let input = host.raw_midi_input_selection().unwrap();
    let playback = host.playback.as_ref().unwrap();
    let advertisements = [
        input
            .input_realization_advertisement(host.advertisement().host_id.clone())
            .unwrap(),
        playback.realization_advertisement(host.advertisement().host_id.clone()),
    ];
    let mut observations = vec![
        input.resource_observation(
            host.advertisement().host_id.clone(),
            conduit_core::SignId::from("sign/instrument-midi-ready"),
        ),
        playback.resource_observation(
            host.advertisement().host_id.clone(),
            conduit_core::SignId::from("sign/instrument-playback-ready"),
        ),
    ];
    if observe_render_clock {
        observations.push(conduit_core::ResourceObservation {
            host_id: host.advertisement().host_id.clone(),
            boot_id: host.advertisement().boot_id.clone(),
            offer_generation: host.advertisement().offer_generation,
            pool_id: conduit_core::ResourcePoolId::from("std/monotonic-deadline"),
            class_id: conduit_core::ResourceClassId::from(
                conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
            ),
            health: conduit_core::ResourceHealth::Ready,
            unreserved_units: 16,
            utilized_units: 0,
            sign_id: conduit_core::SignId::from("sign/instrument-timer-ready"),
        });
    }
    let grants = [
        host.midi_input_authority_grant("grant/instrument-midi")
            .unwrap(),
        host.playback_authority_grant("grant/instrument-playback")
            .unwrap(),
    ];
    let hosts = [host.advertisement().clone()];
    conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        &form,
        conduit_planner::SelectedRealizationPlanning {
            hosts: &hosts,
            bases: &[ConnectionBase::Local],
            requirements: &BTreeMap::new(),
            advertisements: &advertisements,
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            authority_grants: &grants,
        },
    )
    .map(|mut plan| plan.fragments.remove(0))
}

struct StopAfterFourBlocks {
    control: RunControl,
    now_millis: u64,
    request_id: Option<RunControlRequestId>,
}

impl TimerAdapter for StopAfterFourBlocks {
    fn wait(&mut self, duration: Duration) {
        self.now_millis = self.now_millis.saturating_add(duration.as_millis() as u64);
    }

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        if self.now_millis >= 25 && self.request_id.is_some() {
            self.control
                .request_stop(self.request_id.take().unwrap())
                .unwrap();
        }
        Some(self.now_millis)
    }

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        Some(self.now_millis.saturating_mul(1_000))
    }
}

#[test]
fn ordinary_form_runs_midi_synth_and_playback_through_one_kernel() {
    let mut host = host();
    let fragment = fragment(&host);
    assert_eq!(fragment.placements.len(), 4);
    assert_eq!(fragment.connections.len(), 4);
    let synth = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::MUSIC_SYNTH_KIND)
        .unwrap();
    assert_eq!(synth.configuration.len(), 14);
    assert_eq!(synth.resources.len(), 0);
    assert_eq!(synth.authority.len(), 0);
    let input = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::MUSIC_INPUT_KIND)
        .unwrap();
    assert_eq!(input.configuration.len(), 2);
    assert!(input.configuration.iter().any(|entry| {
        entry.key == conduit_std_catalog::MUSIC_INPUT_A4_REFERENCE_KEY
            && entry.value == conduit_core::ConfigurationValue::U64(442_000)
    }));
    assert!(input.configuration.iter().any(|entry| {
        entry.key == conduit_std_catalog::MUSIC_INPUT_TRANSPOSE_KEY
            && entry.value == conduit_core::ConfigurationValue::I64(12)
    }));

    let control = RunControl::default();
    let mut timer = StopAfterFourBlocks {
        control: control.clone(),
        now_millis: 0,
        request_id: Some(RunControlRequestId::new("stop-instrument-proof").unwrap()),
    };
    let report = host
        .run_fragment_controlled_to(
            fragment,
            &mut Vec::with_capacity(2_048),
            &mut timer,
            &control,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(kernel.midi_input.len(), 1);
    assert_eq!(
        kernel.midi_input[0].lifecycle,
        MidiInputLifecycle::CancelledClosed
    );
    assert_eq!(kernel.playback.len(), 1);
    assert_eq!(
        kernel.playback[0].lifecycle,
        PlaybackLifecycle::StoppedClosed
    );
    assert_eq!(kernel.playback[0].metrics.blocks_committed, 4);
    assert_eq!(kernel.playback[0].metrics.frames_committed, 960);
    assert_eq!(kernel.playback[0].metrics.underruns, 0);
}

#[test]
fn render_clock_requires_fresh_current_resource_truth() {
    let host = host();
    assert!(matches!(
        plan_fragment(&host, false),
        Err(conduit_planner::PlannerError::CurrentResourceObservationUnavailable(_))
    ));
}
