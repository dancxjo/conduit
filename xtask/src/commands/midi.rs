use crate::cli::GlobalOpts;
use conduit_std_host::hosted_midi::{discover_alsa_sequencer_endpoints, MidiEndpointDirection};

pub fn list(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        return Err("--json is not yet supported by hosted MIDI commands".into());
    }
    let observations = discover_alsa_sequencer_endpoints()?;
    if observations.is_empty() {
        return Err("no non-system ALSA sequencer endpoints are currently observed".into());
    }
    if !opts.quiet {
        println!("fresh hosted MIDI observations (no sequencer port opened or subscribed):");
        for observation in observations {
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
    }
    Ok(())
}
