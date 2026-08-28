use std::fmt;
use std::process::Command;

const MAXIMUM_ENDPOINTS: usize = 64;
const MAXIMUM_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MidiEndpointDirection {
    ReadableSource,
    WritableDestination,
}

impl MidiEndpointDirection {
    pub(super) const fn identity_segment(self) -> &'static str {
        match self {
            Self::ReadableSource => "readable-source",
            Self::WritableDestination => "writable-destination",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MidiEndpointObservation {
    pub client: u16,
    pub port: u16,
    pub client_name: String,
    pub port_name: String,
    pub client_type: String,
    pub direction: MidiEndpointDirection,
}

#[derive(Debug)]
pub enum MidiDiscoveryError {
    Command(std::io::Error),
    CommandFailed(String),
    InvalidUtf8,
    Malformed(String),
    CapacityExceeded,
}

impl fmt::Display for MidiDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(formatter, "run ALSA sequencer discovery: {error}"),
            Self::CommandFailed(detail) => {
                write!(formatter, "ALSA sequencer discovery failed: {detail}")
            }
            Self::InvalidUtf8 => formatter.write_str("ALSA sequencer discovery was not UTF-8"),
            Self::Malformed(detail) => write!(formatter, "malformed ALSA sequencer list: {detail}"),
            Self::CapacityExceeded => formatter.write_str("ALSA MIDI endpoint capacity exceeded"),
        }
    }
}

impl std::error::Error for MidiDiscoveryError {}

/// Enumerates ALSA sequencer ports through metadata-only list operations.
/// Neither command subscribes, opens a stream, nor grants MIDI authority.
pub fn discover_alsa_sequencer_endpoints(
) -> Result<Vec<MidiEndpointObservation>, MidiDiscoveryError> {
    let mut observations = run_list("-i", MidiEndpointDirection::ReadableSource)?;
    observations.extend(run_list("-o", MidiEndpointDirection::WritableDestination)?);
    if observations.len() > MAXIMUM_ENDPOINTS {
        return Err(MidiDiscoveryError::CapacityExceeded);
    }
    observations.sort();
    Ok(observations)
}

fn run_list(
    argument: &str,
    direction: MidiEndpointDirection,
) -> Result<Vec<MidiEndpointObservation>, MidiDiscoveryError> {
    let output = Command::new("/usr/bin/aconnect")
        .arg(argument)
        .output()
        .map_err(MidiDiscoveryError::Command)?;
    if !output.status.success() {
        return Err(MidiDiscoveryError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let listing =
        std::str::from_utf8(&output.stdout).map_err(|_| MidiDiscoveryError::InvalidUtf8)?;
    parse_aconnect_list(listing, direction)
}

fn parse_aconnect_list(
    listing: &str,
    direction: MidiEndpointDirection,
) -> Result<Vec<MidiEndpointObservation>, MidiDiscoveryError> {
    let mut observations = Vec::new();
    let mut current: Option<(u16, String, String)> = None;
    for line in listing.lines() {
        if line.starts_with("client ") {
            let parsed = parse_client(line)?;
            current = (parsed.0 != 0).then_some(parsed);
            continue;
        }
        if !line.starts_with("    ") || line.trim_start().starts_with("Connecting") {
            continue;
        }
        let Some((client, client_name, client_type)) = current.as_ref() else {
            continue;
        };
        let (port, port_name) = parse_port(line)?;
        if observations.len() == MAXIMUM_ENDPOINTS {
            return Err(MidiDiscoveryError::CapacityExceeded);
        }
        observations.push(MidiEndpointObservation {
            client: *client,
            port,
            client_name: client_name.clone(),
            port_name,
            client_type: client_type.clone(),
            direction,
        });
    }
    Ok(observations)
}

fn parse_client(line: &str) -> Result<(u16, String, String), MidiDiscoveryError> {
    let rest = line
        .strip_prefix("client ")
        .ok_or_else(|| MidiDiscoveryError::Malformed(line.to_owned()))?;
    let (client, tail) = rest
        .split_once(": ")
        .ok_or_else(|| MidiDiscoveryError::Malformed(line.to_owned()))?;
    let client = client
        .parse::<u16>()
        .map_err(|_| MidiDiscoveryError::Malformed(line.to_owned()))?;
    let client_name = quoted(tail)?;
    let type_start = tail
        .rfind("[type=")
        .ok_or_else(|| MidiDiscoveryError::Malformed(line.to_owned()))?;
    let client_type = tail[type_start + 6..]
        .strip_suffix(']')
        .ok_or_else(|| MidiDiscoveryError::Malformed(line.to_owned()))?;
    validate_name(&client_name, line)?;
    validate_name(client_type, line)?;
    Ok((client, client_name, client_type.to_owned()))
}

fn parse_port(line: &str) -> Result<(u16, String), MidiDiscoveryError> {
    let trimmed = line.trim_start();
    let split = trimmed
        .find(char::is_whitespace)
        .ok_or_else(|| MidiDiscoveryError::Malformed(line.to_owned()))?;
    let port = trimmed[..split]
        .parse::<u16>()
        .map_err(|_| MidiDiscoveryError::Malformed(line.to_owned()))?;
    let port_name = quoted(&trimmed[split..])?;
    validate_name(&port_name, line)?;
    Ok((port, port_name))
}

fn quoted(value: &str) -> Result<String, MidiDiscoveryError> {
    let start = value
        .find('\'')
        .ok_or_else(|| MidiDiscoveryError::Malformed(value.to_owned()))?;
    let end = value
        .rfind('\'')
        .filter(|end| *end > start)
        .ok_or_else(|| MidiDiscoveryError::Malformed(value.to_owned()))?;
    Ok(value[start + 1..end].trim_end().to_owned())
}

fn validate_name(name: &str, source: &str) -> Result<(), MidiDiscoveryError> {
    if name.is_empty() || name.len() > MAXIMUM_NAME_BYTES {
        return Err(MidiDiscoveryError::Malformed(source.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "client 0: 'System' [type=kernel]\n    0 'Timer           '\n    1 'Announce        '\nclient 14: 'Midi Through' [type=kernel]\n    0 'Midi Through Port-0'\n\tConnecting To: 142:0\nclient 28: 'Keyboard' [type=user,pid=42]\n    1 'Keys'\n";

    #[test]
    fn parses_exact_ports_but_excludes_alsa_system_services() {
        let ports = parse_aconnect_list(LISTING, MidiEndpointDirection::ReadableSource).unwrap();
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].client, 14);
        assert_eq!(ports[0].port, 0);
        assert_eq!(ports[0].client_name, "Midi Through");
        assert_eq!(ports[0].port_name, "Midi Through Port-0");
        assert_eq!(ports[0].client_type, "kernel");
        assert_eq!(ports[1].client, 28);
        assert_eq!(ports[1].client_type, "user,pid=42");
    }

    #[test]
    fn empty_listing_is_exactly_no_endpoint() {
        assert!(
            parse_aconnect_list("", MidiEndpointDirection::WritableDestination)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn malformed_identity_fails_closed() {
        assert!(matches!(
            parse_aconnect_list(
                "client nope: 'Keyboard' [type=kernel]\n",
                MidiEndpointDirection::ReadableSource
            ),
            Err(MidiDiscoveryError::Malformed(_))
        ));
    }
}
