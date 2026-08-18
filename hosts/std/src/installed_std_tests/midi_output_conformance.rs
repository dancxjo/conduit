use super::RecordingTimer;
use crate::hosted_midi::{
    output_fake::FakeMidiOutputBehavior, HostedMidiSelection, MidiEndpointDirection,
    MidiEndpointObservation, MidiOutputLifecycle,
};
use crate::{RunControl, RunControlRequestId, StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{
    BootId, ConnectionBase, HostId, ObservationKind, OfferGeneration, TerminalDisposition,
};
use conduit_std_catalog::{NormalizedSoundTrace, RealizedSoundEvidence, SelectedSoundRealization};
use std::collections::BTreeMap;

fn host() -> StdHost {
    let config = StdHostConfig {
        host_id: HostId::from("midi-loopback-host"),
        boot_id: BootId::from("midi-loopback-boot"),
        offer_generation: OfferGeneration(9),
    };
    let observation = MidiEndpointObservation {
        client: 24,
        port: 1,
        client_name: "Deterministic loopback".into(),
        port_name: "Writable endpoint".into(),
        client_type: "user".into(),
        direction: MidiEndpointDirection::WritableDestination,
    };
    let selection = HostedMidiSelection::select(
        &[observation],
        MidiEndpointDirection::WritableDestination,
        24,
        1,
        config.boot_id.clone(),
        config.offer_generation,
    )
    .unwrap()
    .with_fake_output(FakeMidiOutputBehavior::Healthy);
    StdHost::new_with_midi_output(config, StdHostComposition::minimal(), selection)
        .expect("loopback selection matches exact Host identity")
}

fn form() -> conduit_form::CheckedForm {
    conduit_form::parse(
        "form midi_loopback {\n source: conduit-proof/midi-performance-source\n output: music/play\n source.notes > output.notes\n source.controls > output.controls\n}\n",
        &crate::installed_std::test_catalog(),
    )
    .expect("portable MIDI loopback Form is valid")
}

fn fragment(host: &StdHost, grant_count: usize) -> Result<conduit_core::PlanFragment, String> {
    let form = form();
    let hosts = [host.advertisement().clone()];
    let grants = host
        .midi_output_authority_grants("grant/test-midi-output")?
        .into_iter()
        .take(grant_count)
        .collect::<Vec<_>>();
    let selected = host
        .midi_output_selection()
        .expect("fixture Host has selected MIDI output");
    let realization = selected
        .output_realization_advertisement(host.advertisement().host_id.clone())
        .unwrap();
    let observation = selected.resource_observation(
        host.advertisement().host_id.clone(),
        conduit_core::SignId::from("sign/test-midi-output-ready"),
    );
    let plan = conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        &form,
        conduit_planner::SelectedRealizationPlanning {
            hosts: &hosts,
            bases: &[ConnectionBase::Local],
            requirements: &BTreeMap::new(),
            advertisements: &[realization],
            observations: &[observation],
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::NOTE_EVENT_ENCODED_LEN as u32,
            authority_grants: &grants,
        },
    )
    .map_err(|error| format!("plan MIDI loopback: {error:?}"))?;
    Ok(plan.fragments[0].clone())
}

#[test]
fn planned_portable_performance_runs_through_the_production_kernel() {
    let mut host = host();
    let fragment = fragment(&host, 2).unwrap();
    let output = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::MUSIC_PLAY_KIND)
        .unwrap();
    assert_eq!(output.resources.len(), 1);
    assert_eq!(output.authority.len(), 2);
    assert_eq!(output.host_operations.len(), 2);
    assert_eq!(fragment.connections.len(), 2);
    assert!(fragment
        .connections
        .iter()
        .all(|connection| connection.item_capacity == 1));
    let expected_plan_id = fragment.plan_id.clone();
    let selected = SelectedSoundRealization {
        plan_id: fragment.plan_id.clone(),
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        implementation_id: output.implementation_id.clone(),
    };

    let report = host
        .run_fragment_to(
            fragment,
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect("portable performance executes through installed production kernel");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.midi_output.len(), 1);
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.midi_output[0].encoded_messages,
        vec![
            [0x90, 69, 127],
            [0xb0, 64, 127],
            [0x80, 69, 0],
            [0xb0, 123, 0]
        ]
    );
    assert_eq!(
        kernel.midi_output[0].lifecycle,
        MidiOutputLifecycle::StoppedClosed
    );
    let evidence = RealizedSoundEvidence {
        selected,
        trace: NormalizedSoundTrace::new(
            kernel.midi_output[0].normalized_note_events.clone(),
            TerminalDisposition::Completed,
        )
        .unwrap(),
    };
    assert_eq!(evidence.selected.plan_id, expected_plan_id);
    assert_eq!(
        evidence.selected.implementation_id.as_str(),
        conduit_std_catalog::MUSIC_PLAY_MIDI_IMPLEMENTATION
    );
    assert_eq!(evidence.trace.events[0].occurrence, 41);
    assert_eq!(evidence.trace.events[0].admitted_pitch_millihertz, 440_000);
    assert_eq!(evidence.trace.terminal, TerminalDisposition::Completed);
}

#[test]
fn missing_typed_authority_refuses_before_play() {
    let host = host();
    let error = fragment(&host, 1).unwrap_err();
    assert!(error.contains("Authority") || error.contains("authority"));
}

#[test]
fn stop_before_first_event_closes_with_only_all_notes_off() {
    let mut host = host();
    let fragment = fragment(&host, 2).unwrap();
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("stop-midi-before-first-event").unwrap())
        .unwrap();
    let report = host
        .run_fragment_controlled_to(
            fragment,
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
            &control,
        )
        .expect("cancelled MIDI Play closes its admitted session");
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.midi_output[0].encoded_messages, vec![[0xb0, 123, 0]]);
    assert!(kernel.midi_output[0].all_notes_off_sent);
}
