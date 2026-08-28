use super::MidiEndpointDirection;
use std::fmt;
use std::process::Command;

pub const MAXIMUM_RAW_MIDI_ENDPOINTS: usize = 64;
const MAXIMUM_RAW_MIDI_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMidiEndpointObservation {
    pub card: u16,
    pub device: u16,
    pub subdevice: u16,
    pub name: String,
    pub direction: MidiEndpointDirection,
}

impl RawMidiEndpointObservation {
    pub fn alsa_device_name(&self) -> String {
        format!("hw:{},{},{}", self.card, self.device, self.subdevice)
    }

    pub fn direct_device_path(&self) -> Option<String> {
        (self.subdevice == 0).then(|| format!("/dev/snd/midiC{}D{}", self.card, self.device))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawMidiDiscoveryError {
    ToolUnavailable,
    QueryFailed(i32),
    NonUtf8,
    MalformedRow(String),
    NameTooLong,
    CapacityExceeded,
}

impl fmt::Display for RawMidiDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "raw MIDI discovery failed: {self:?}")
    }
}

impl std::error::Error for RawMidiDiscoveryError {}

/// Lists ALSA RawMIDI metadata without opening a RawMIDI stream or granting
/// authority to one.
pub fn discover_raw_midi_endpoints(
) -> Result<Vec<RawMidiEndpointObservation>, RawMidiDiscoveryError> {
    let output = Command::new("/usr/bin/amidi")
        .arg("-l")
        .output()
        .map_err(|_| RawMidiDiscoveryError::ToolUnavailable)?;
    if !output.status.success() {
        return Err(RawMidiDiscoveryError::QueryFailed(
            output.status.code().unwrap_or(-1),
        ));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_| RawMidiDiscoveryError::NonUtf8)?;
    parse_raw_midi_listing(&stdout)
}

fn parse_raw_midi_listing(
    listing: &str,
) -> Result<Vec<RawMidiEndpointObservation>, RawMidiDiscoveryError> {
    let mut observations = Vec::new();
    for line in listing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line.starts_with("Dir ") {
            continue;
        }
        let mut columns = line.split_whitespace();
        let directions = columns
            .next()
            .ok_or_else(|| RawMidiDiscoveryError::MalformedRow(line.into()))?;
        let device_name = columns
            .next()
            .ok_or_else(|| RawMidiDiscoveryError::MalformedRow(line.into()))?;
        let name = columns.collect::<Vec<_>>().join(" ");
        if name.is_empty() || name.len() > MAXIMUM_RAW_MIDI_NAME_BYTES {
            return Err(if name.is_empty() {
                RawMidiDiscoveryError::MalformedRow(line.into())
            } else {
                RawMidiDiscoveryError::NameTooLong
            });
        }
        let (card, device, subdevice) = parse_device_name(device_name)
            .ok_or_else(|| RawMidiDiscoveryError::MalformedRow(line.into()))?;
        let mut emit = |direction| {
            if observations.len() >= MAXIMUM_RAW_MIDI_ENDPOINTS {
                return Err(RawMidiDiscoveryError::CapacityExceeded);
            }
            observations.push(RawMidiEndpointObservation {
                card,
                device,
                subdevice,
                name: name.clone(),
                direction,
            });
            Ok(())
        };
        let mut recognized = false;
        if directions.contains('I') {
            emit(MidiEndpointDirection::ReadableSource)?;
            recognized = true;
        }
        if directions.contains('O') {
            emit(MidiEndpointDirection::WritableDestination)?;
            recognized = true;
        }
        if !recognized || directions.chars().any(|item| item != 'I' && item != 'O') {
            return Err(RawMidiDiscoveryError::MalformedRow(line.into()));
        }
    }
    observations.sort_by_key(|item| {
        (
            item.card,
            item.device,
            item.subdevice,
            item.direction.identity_segment(),
        )
    });
    Ok(observations)
}

fn parse_device_name(device: &str) -> Option<(u16, u16, u16)> {
    let mut fields = device.strip_prefix("hw:")?.split(',');
    let card = fields.next()?.parse().ok()?;
    let device = fields.next()?.parse().ok()?;
    let subdevice = fields.next()?.parse().ok()?;
    fields.next().is_none().then_some((card, device, subdevice))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplex_row_becomes_two_exact_directional_observations() {
        let observations = parse_raw_midi_listing(
            "Dir Device    Name\n IO hw:2,0,0  USB Keyboard MIDI 1\n  O hw:3,1,0  Synth Output\n",
        )
        .unwrap();
        assert_eq!(observations.len(), 3);
        assert_eq!(observations[0].alsa_device_name(), "hw:2,0,0");
        assert_eq!(
            observations[0].direct_device_path().as_deref(),
            Some("/dev/snd/midiC2D0")
        );
        assert_eq!(
            observations[0].direction,
            MidiEndpointDirection::ReadableSource
        );
        assert_eq!(
            observations[1].direction,
            MidiEndpointDirection::WritableDestination
        );
        assert_eq!(observations[2].name, "Synth Output");
    }

    #[test]
    fn malformed_coordinate_direction_and_capacity_fail_closed() {
        for listing in [
            "Dir Device Name\n O default Device\n",
            "Dir Device Name\n X hw:1,0,0 Device\n",
            "Dir Device Name\n O hw:1,0 Device\n",
        ] {
            assert!(matches!(
                parse_raw_midi_listing(listing),
                Err(RawMidiDiscoveryError::MalformedRow(_))
            ));
        }
        let listing = (0..=MAXIMUM_RAW_MIDI_ENDPOINTS)
            .map(|index| format!("O hw:{index},0,0 Device {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            parse_raw_midi_listing(&listing),
            Err(RawMidiDiscoveryError::CapacityExceeded)
        );
    }

    #[test]
    fn subdevice_does_not_alias_a_direct_device_node() {
        let observation = parse_raw_midi_listing("O hw:1,2,3 Multi Subdevice\n")
            .unwrap()
            .remove(0);
        assert_eq!(observation.alsa_device_name(), "hw:1,2,3");
        assert_eq!(observation.direct_device_path(), None);
    }
}
