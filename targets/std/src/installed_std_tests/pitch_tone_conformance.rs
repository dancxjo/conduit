use super::RecordingTimer;
use crate::hosted_audio::{
    AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection, PlaybackLifecycle,
};
use crate::{RunControl, RunControlRequestId, StdHost, StdHostComposition, StdHostConfig};
use conduit_audio::PcmSampleRepresentation;
use conduit_core::{
    BaseImplementationId, BootId, HostId, ObservationKind, OfferGeneration, TerminalDisposition,
};
use std::collections::BTreeMap;

fn host() -> StdHost {
    let config = StdHostConfig {
        host_id: HostId::from("pitch-tone-fixture-host"),
        boot_id: BootId::from("pitch-tone-fixture-boot"),
        offer_generation: OfferGeneration(41),
    };
    let selection = HostedPlaybackSelection::deterministic_fake_with_capture(
        AlsaPlaybackObservation {
            card_index: 0,
            card_id: "FIXTURE".into(),
            card_name: "Pitch tone fixture".into(),
            device: 0,
            device_name: "Finite PCM sink".into(),
            base_identity: "pitch-tone-fixture-base".into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
        FakePlaybackBehavior::Success,
    );
    StdHost::new_with_playback(config, StdHostComposition::reference(), selection)
        .expect("pitch-tone fixture selection matches exact host identity")
}

fn form(target_hz: i64) -> conduit_form::CheckedForm {
    conduit_form::parse(
        &format!(
            "form pitch_tone_fixture {{\n source: conduit-test/scalar-literal\n map: math/map-quantity(source-minimum = -1000000, source-maximum = 1000000, target-minimum = {target_hz}, target-maximum = {target_hz}, target-granularity = 1, unit = \"Hz\", range-policy = \"clamp\", quantization = \"exact\")\n tone: sound/pitch-tone\n source.value > map.in\n map.out > tone.pitch\n}}\n"
        ),
        &crate::installed_std::test_catalog(),
    )
    .expect("pitch-tone fixture form is valid")
}

fn fragment(
    host: &StdHost,
    with_authority: bool,
    target_hz: i64,
) -> Result<conduit_core::PlanFragment, String> {
    let form = form(target_hz);
    let advertisements = [host.advertisement().clone()];
    let grants = if with_authority {
        vec![host.pitch_tone_authority_grant("grant/test-pitch-tone")?]
    } else {
        Vec::new()
    };
    let playback = host.playback.as_ref().expect("fixture host has playback");
    let realization = playback.realization_advertisement(host.advertisement().host_id.clone());
    let observations = vec![playback.resource_observation(
        host.advertisement().host_id.clone(),
        conduit_core::SignId::from("sign/test-pitch-tone-ready"),
    )];
    let plan = conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        &form,
        conduit_planner::SelectedRealizationPlanning {
            hosts: &advertisements,
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &[realization],
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::QUANTITY_ENCODED_LEN as u32,
            authority_grants: &grants,
        },
    )
    .map_err(|error| format!("plan pitch-tone fixture: {error:?}"))?;
    Ok(plan.fragments[0].clone())
}

#[test]
fn hosted_pitch_tone_offer_and_playback_bounds_are_exact() {
    let host = host();
    let offer = host
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::PITCH_TONE_KIND)
        .unwrap();
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        crate::installed_std::PITCH_TONE_IMPLEMENTATION
    );
    assert_eq!(offer.inputs.len(), 1);
    assert!(offer.outputs.is_empty());
    assert_eq!(offer.host_calls.len(), 1);
    assert_eq!(
        offer.host_calls[0].maximum_input_bytes,
        conduit_core::QUANTITY_ENCODED_LEN as u32
    );
    assert_eq!(offer.host_calls[0].maximum_output_bytes, 0);
    assert_eq!(
        offer.host_calls[0].target_kind.as_ref().unwrap().as_str(),
        conduit_audio::AUDIO_PCM_INFO_ID
    );
    assert_eq!(offer.resource_requirements.len(), 1);
    assert_eq!(
        offer.resource_requirements[0].class_id.as_str(),
        conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS
    );
    assert_eq!(offer.authority_requirements.len(), 1);
}

#[test]
fn scalar_to_quantity_to_pitch_tone_runs_through_the_installed_kernel_with_bounded_pcm() {
    let mut low_host = host();
    let plan_fragment = fragment(&low_host, true, 220).unwrap();
    let tone = plan_fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::PITCH_TONE_KIND)
        .unwrap();
    assert_eq!(
        tone.implementation_id.as_str(),
        crate::installed_std::PITCH_TONE_IMPLEMENTATION
    );
    assert_eq!(tone.resources.len(), 1);
    assert_eq!(tone.authority.len(), 1);
    assert_eq!(plan_fragment.connections.len(), 2);
    assert_eq!(plan_fragment.connections[1].item_capacity, 1);
    assert_eq!(
        plan_fragment.connections[1].byte_capacity,
        conduit_core::QUANTITY_ENCODED_LEN as u32
    );

    let low_report = low_host
        .run_fragment_to(
            plan_fragment,
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap();
    let mut high_host = host();
    let high_report = high_host
        .run_fragment_to(
            fragment(&high_host, true, 880).unwrap(),
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap();
    assert!(matches!(
        low_report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert!(matches!(
        high_report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = low_report.kernel.unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    let playback = &kernel.playback[0];
    assert_eq!(playback.lifecycle, PlaybackLifecycle::StoppedClosed);
    assert_eq!(
        playback.metrics.blocks_committed,
        u32::from(crate::installed_std::PITCH_TONE_BLOCKS)
    );
    assert_eq!(
        playback.metrics.frames_committed,
        u64::from(playback.metrics.blocks_committed)
            * u64::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES)
    );
    assert_eq!(
        playback.committed_headers.len(),
        usize::from(crate::installed_std::PITCH_TONE_BLOCKS)
    );
    assert_eq!(
        playback.committed_payload_digests.len(),
        usize::from(crate::installed_std::PITCH_TONE_BLOCKS)
    );
    let first = playback.committed_headers[0];
    assert_eq!(
        first.representation,
        PcmSampleRepresentation::Signed16LittleEndian
    );
    assert_eq!(first.sample_rate_hz, crate::hosted_audio::SAMPLE_RATE_HZ);
    assert_eq!(
        first.layout,
        conduit_audio::PcmChannelLayout::StereoLeftRight
    );
    assert_eq!(
        first.frame_count,
        conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES
    );
    assert!(first.discontinuity);
    let high_playback = &high_report.kernel.unwrap().playback[0];
    assert_eq!(
        high_playback.metrics.blocks_committed,
        u32::from(crate::installed_std::PITCH_TONE_BLOCKS)
    );
    assert_ne!(
        playback.committed_payload_digests[0],
        high_playback.committed_payload_digests[0]
    );
}

#[test]
fn cancellation_before_first_frequency_leaves_pitch_tone_playback_closed_and_empty() {
    let mut host = host();
    let fragment = fragment(&host, true, 220).unwrap();
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("cancel-before-pitch-tone").unwrap())
        .unwrap();
    let report = host
        .run_fragment_controlled_to(
            fragment,
            &mut Vec::with_capacity(1_024),
            &mut RecordingTimer { waits: Vec::new() },
            &control,
        )
        .expect("pre-open cancellation remains a successful control operation");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    let playback = &report.kernel.unwrap().playback[0];
    assert_eq!(playback.lifecycle, PlaybackLifecycle::StoppedClosed);
    assert_eq!(playback.metrics.blocks_committed, 0);
    assert_eq!(playback.metrics.frames_committed, 0);
    assert!(playback.committed_headers.is_empty());
    assert!(playback.committed_payload_digests.is_empty());
}

#[test]
fn missing_playback_authority_refuses_pitch_tone_planning() {
    let host = host();
    let error = fragment(&host, false, 220).unwrap_err();
    assert!(error.contains("AuthorityGrantMissing"), "{error}");
}
