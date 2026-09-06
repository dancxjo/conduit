use crate::cli::GlobalOpts;
use crate::output::{RepositoryOutput, MAXIMUM_OUTPUT_ITEMS};
use conduit_std_host::hosted_midi::{
    discover_alsa_sequencer_endpoints, discover_raw_midi_endpoints, MidiEndpointDirection,
};
use serde::Serialize;

const MIDI_LIST_SCHEMA: &str = "conduit.tools/xtask/hosted-midi-list@1";

#[derive(Serialize)]
struct MidiListReport<'a> {
    schema: &'static str,
    dry_run: bool,
    effects_performed: bool,
    sequencer: Vec<MidiEndpointReport<'a>>,
    raw: Vec<RawMidiEndpointReport<'a>>,
}

#[derive(Serialize)]
struct MidiEndpointReport<'a> {
    direction: &'static str,
    client: u16,
    port: u16,
    client_type: &'a str,
    client_name: &'a str,
    port_name: &'a str,
}

#[derive(Serialize)]
struct RawMidiEndpointReport<'a> {
    direction: &'static str,
    device: String,
    direct_path: Option<String>,
    name: &'a str,
}

pub fn list(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let output = RepositoryOutput::from_opts(opts);
    if output.dry_run() {
        output.emit_json(&MidiListReport {
            schema: MIDI_LIST_SCHEMA,
            dry_run: true,
            effects_performed: false,
            sequencer: Vec::new(),
            raw: Vec::new(),
        })?;
        output.emit_human(|writer| {
            writeln!(
                writer,
                "dry-run hosted MIDI list: metadata discovery not performed"
            )
        })?;
        return Ok(());
    }
    let sequencer = discover_alsa_sequencer_endpoints()?;
    let raw = discover_raw_midi_endpoints()?;
    if sequencer.len() > MAXIMUM_OUTPUT_ITEMS || raw.len() > MAXIMUM_OUTPUT_ITEMS {
        return Err("hosted MIDI observation output capacity exceeded".into());
    }
    if sequencer.is_empty() && raw.is_empty() {
        return Err(
            "no non-system ALSA sequencer or RawMIDI endpoints are currently observed".into(),
        );
    }
    output.emit_json(&MidiListReport {
        schema: MIDI_LIST_SCHEMA,
        dry_run: false,
        effects_performed: false,
        sequencer: sequencer
            .iter()
            .map(|observation| MidiEndpointReport {
                direction: direction(observation.direction),
                client: observation.client,
                port: observation.port,
                client_type: &observation.client_type,
                client_name: &observation.client_name,
                port_name: &observation.port_name,
            })
            .collect(),
        raw: raw
            .iter()
            .map(|observation| RawMidiEndpointReport {
                direction: direction(observation.direction),
                device: observation.alsa_device_name(),
                direct_path: observation.direct_device_path(),
                name: &observation.name,
            })
            .collect(),
    })?;
    output.emit_human(|writer| {
        writeln!(
            writer,
            "fresh hosted MIDI observations (no sequencer port opened or subscribed):"
        )?;
        for observation in &sequencer {
            let direction = match observation.direction {
                MidiEndpointDirection::ReadableSource => "readable-source",
                MidiEndpointDirection::WritableDestination => "writable-destination",
            };
            writeln!(
                writer,
                "direction={} address={}:{} client-type={:?} client-name={:?} port-name={:?}",
                direction,
                observation.client,
                observation.port,
                observation.client_type,
                observation.client_name,
                observation.port_name,
            )?;
        }
        writeln!(
            writer,
            "fresh raw MIDI byte endpoints (no RawMIDI stream opened):"
        )?;
        if raw.is_empty() {
            writeln!(writer, "none")?;
        }
        for observation in &raw {
            let direction = match observation.direction {
                MidiEndpointDirection::ReadableSource => "readable-source",
                MidiEndpointDirection::WritableDestination => "writable-destination",
            };
            writeln!(
                writer,
                "direction={} device={} direct-path={:?} name={:?}",
                direction,
                observation.alsa_device_name(),
                observation.direct_device_path(),
                observation.name,
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

const fn direction(value: MidiEndpointDirection) -> &'static str {
    match value {
        MidiEndpointDirection::ReadableSource => "readable-source",
        MidiEndpointDirection::WritableDestination => "writable-destination",
    }
}
