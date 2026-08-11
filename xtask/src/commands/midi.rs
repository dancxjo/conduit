use crate::cli::GlobalOpts;
use conduit_std_host::hosted_midi::{
    discover_alsa_sequencer_endpoints, discover_raw_midi_endpoints, MidiEndpointDirection,
};

pub fn list(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        return Err("--json is not yet supported by hosted MIDI commands".into());
    }
    let sequencer = discover_alsa_sequencer_endpoints()?;
    let raw = discover_raw_midi_endpoints()?;
    if sequencer.is_empty() && raw.is_empty() {
        return Err(
            "no non-system ALSA sequencer or RawMIDI endpoints are currently observed".into(),
        );
    }
    if !opts.quiet {
        println!("fresh hosted MIDI observations (no sequencer port opened or subscribed):");
        for observation in sequencer {
            let direction = match observation.direction {
                MidiEndpointDirection::ReadableSource => "readable-source",
                MidiEndpointDirection::WritableDestination => "writable-destination",
            };
            println!(
                "direction={} address={}:{} client-type={:?} client-name={:?} port-name={:?}",
                direction,
                observation.client,
                observation.port,
                observation.client_type,
                observation.client_name,
                observation.port_name,
            );
        }
        println!("fresh raw MIDI byte endpoints (no RawMIDI stream opened):");
        if raw.is_empty() {
            println!("none");
        }
        for observation in raw {
            let direction = match observation.direction {
                MidiEndpointDirection::ReadableSource => "readable-source",
                MidiEndpointDirection::WritableDestination => "writable-destination",
            };
            println!(
                "direction={} device={} direct-path={:?} name={:?}",
                direction,
                observation.alsa_device_name(),
                observation.direct_device_path(),
                observation.name,
            );
        }
    }
    Ok(())
}
