use crate::hosted_midi::{
    output_fake::FakeMidiOutputBehavior, HostedMidiSelection, HostedRawMidiSelection,
    MidiEndpointDirection, MidiEndpointObservation, MidiInputLifecycle, MidiOutputLifecycle,
    RawMidiEndpointObservation,
};
use crate::{
    RunControl, RunControlRequestId, StdHost, StdHostComposition, StdHostConfig, TimerAdapter,
};
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration, TerminalDisposition};
use std::collections::BTreeMap;
use std::time::Duration;

fn host() -> StdHost {
    host_with_input(
        vec![
            0x90, 60, 100, // note on
            0xb0, 64, 127, // sustain
            0x80, 60, 0,    // note off
            0xf8, // bounded unsupported observation used as the Stop boundary
        ],
        false,
    )
}

fn host_with_input(bytes: Vec<u8>, disconnect_after_input: bool) -> StdHost {
    let config = StdHostConfig {
        host_id: HostId::from("midi-input-loopback-host"),
        boot_id: BootId::from("midi-input-loopback-boot"),
        offer_generation: OfferGeneration(12),
    };
    let input = HostedRawMidiSelection::select(
        &[RawMidiEndpointObservation {
            card: u16::MAX,
            device: u16::MAX,
            subdevice: 0,
            name: "Bounded input fixture".into(),
            direction: MidiEndpointDirection::ReadableSource,
        }],
        MidiEndpointDirection::ReadableSource,
        u16::MAX,
        u16::MAX,
        0,
        config.boot_id.clone(),
        config.offer_generation,
    )
    .unwrap();
    let input = if disconnect_after_input {
        input.with_fake_input_then_disconnect(bytes)
    } else {
        input.with_fake_input(bytes)
    };
    let output = HostedMidiSelection::select(
        &[MidiEndpointObservation {
            client: 42,
            port: 3,
            client_name: "Bounded output fixture".into(),
            port_name: "Loopback".into(),
            client_type: "user".into(),
            direction: MidiEndpointDirection::WritableDestination,
        }],
        MidiEndpointDirection::WritableDestination,
        42,
        3,
        config.boot_id.clone(),
        config.offer_generation,
    )
    .unwrap()
    .with_fake_output(FakeMidiOutputBehavior::Healthy);
    StdHost::new_with_raw_midi_input_and_midi_output(
        config,
        StdHostComposition::minimal(),
        input,
        output,
    )
    .unwrap()
}

struct AdvancingTimer(u64);

impl TimerAdapter for AdvancingTimer {
    fn wait(&mut self, _duration: Duration) {}

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        self.0 += 100;
        Some(self.0)
    }
}

fn fragment(host: &StdHost) -> conduit_core::PlanFragment {
    let form = conduit_form::parse(
        "form input_loopback {\n input: music/input\n output: music/play\n input.notes > output.notes\n input.controls > output.controls\n}\n",
        &crate::installed_std::test_catalog(),
    )
    .unwrap();
    let input = host.raw_midi_input_selection().unwrap();
    let output = host.midi_output_selection().unwrap();
    let advertisements = [
        input
            .input_realization_advertisement(host.advertisement().host_id.clone())
            .unwrap(),
        output
            .output_realization_advertisement(host.advertisement().host_id.clone())
            .unwrap(),
    ];
    let observations = [
        input.resource_observation(
            host.advertisement().host_id.clone(),
            conduit_core::SignId::from("sign/midi-input-ready"),
        ),
        output.resource_observation(
            host.advertisement().host_id.clone(),
            conduit_core::SignId::from("sign/midi-output-ready"),
        ),
    ];
    let mut grants = vec![host.midi_input_authority_grant("grant/midi-input").unwrap()];
    grants.extend(
        host.midi_output_authority_grants("grant/midi-output")
            .unwrap(),
    );
    let hosts = [host.advertisement().clone()];
    conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        &form,
        conduit_planner::SelectedRealizationPlanning {
            hosts: &hosts,
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &advertisements,
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
            authority_grants: &grants,
        },
    )
    .unwrap()
    .fragments
    .remove(0)
}

struct StopAfterFourthObservation {
    control: RunControl,
    readings: u8,
    request_id: Option<RunControlRequestId>,
}

impl TimerAdapter for StopAfterFourthObservation {
    fn wait(&mut self, _duration: Duration) {}

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        self.readings += 1;
        if self.readings == 4 {
            self.control
                .request_stop(self.request_id.take().unwrap())
                .unwrap();
        }
        Some(u64::from(self.readings) * 100)
    }
}

#[test]
fn authored_input_reaches_portable_ports_and_output_through_one_kernel() {
    let mut host = host();
    let fragment = fragment(&host);
    assert_eq!(fragment.connections.len(), 2);
    assert!(fragment
        .connections
        .iter()
        .all(|connection| connection.item_capacity == 1));
    let control = RunControl::default();
    let mut timer = StopAfterFourthObservation {
        control: control.clone(),
        readings: 0,
        request_id: Some(RunControlRequestId::new("stop-after-midi-proof").unwrap()),
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
    assert_eq!(kernel.midi_input[0].observations, 4);
    assert_eq!(kernel.midi_input[0].pending_bytes, 0);
    assert_eq!(kernel.midi_output.len(), 1);
    assert_eq!(
        kernel.midi_output[0].lifecycle,
        MidiOutputLifecycle::StoppedClosed
    );
    assert_eq!(
        kernel.midi_output[0].encoded_messages,
        vec![
            [0x90, 60, 100],
            [0xb0, 64, 127],
            [0x80, 60, 0],
            [0xb0, 123, 0],
        ]
    );
}

#[test]
fn provider_loss_with_an_active_note_fails_the_semantic_run() {
    let mut host = host_with_input(vec![0x90, 60, 100], true);
    let fragment = fragment(&host);
    let error = host
        .run_fragment_controlled_to(
            fragment,
            &mut Vec::with_capacity(2_048),
            &mut AdvancingTimer(0),
            &RunControl::default(),
        )
        .expect_err("provider loss must terminate the semantic run");

    assert!(
        error.contains("OperationFailed(Failure { code: HostOperationFailed, detail: 103 })"),
        "{error}"
    );
}
