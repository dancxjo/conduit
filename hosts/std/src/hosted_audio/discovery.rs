use std::fmt;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlsaPlaybackObservation {
    pub card_index: u16,
    pub card_id: String,
    pub card_name: String,
    pub device: u16,
    pub device_name: String,
    pub base_identity: String,
}

#[derive(Debug)]
pub enum PlaybackDiscoveryError {
    Command(std::io::Error),
    CommandFailed(String),
    InvalidUtf8,
    Malformed(String),
}

impl fmt::Display for PlaybackDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(formatter, "run aplay discovery: {error}"),
            Self::CommandFailed(detail) => write!(formatter, "aplay discovery failed: {detail}"),
            Self::InvalidUtf8 => formatter.write_str("aplay discovery was not UTF-8"),
            Self::Malformed(detail) => write!(formatter, "malformed aplay discovery: {detail}"),
        }
    }
}

impl std::error::Error for PlaybackDiscoveryError {}

/// Enumerates playback resources without opening any PCM handle. `aplay -l`
/// reads ALSA control metadata only; actual usability is rechecked at Start.
pub fn discover_alsa_playback() -> Result<Vec<AlsaPlaybackObservation>, PlaybackDiscoveryError> {
    let output = Command::new("/usr/bin/aplay")
        .arg("-l")
        .output()
        .map_err(PlaybackDiscoveryError::Command)?;
    if !output.status.success() {
        return Err(PlaybackDiscoveryError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let stdout =
        std::str::from_utf8(&output.stdout).map_err(|_| PlaybackDiscoveryError::InvalidUtf8)?;
    parse_aplay_list(stdout, Path::new("/sys/class/sound"))
}

fn parse_aplay_list(
    listing: &str,
    sound_class: &Path,
) -> Result<Vec<AlsaPlaybackObservation>, PlaybackDiscoveryError> {
    let mut observations = Vec::new();
    for line in listing.lines().filter(|line| line.starts_with("card ")) {
        let (card_part, device_part) = line.split_once(", device ").ok_or_else(|| {
            PlaybackDiscoveryError::Malformed(format!("missing device delimiter in {line:?}"))
        })?;
        let card_part = card_part
            .strip_prefix("card ")
            .ok_or_else(|| PlaybackDiscoveryError::Malformed(line.to_owned()))?;
        let (card_index, card_tail) = card_part.split_once(": ").ok_or_else(|| {
            PlaybackDiscoveryError::Malformed(format!("missing card delimiter in {line:?}"))
        })?;
        let (card_id, card_name) = bracketed_name(card_tail)?;
        let (device, device_tail) = device_part.split_once(": ").ok_or_else(|| {
            PlaybackDiscoveryError::Malformed(format!("missing device name in {line:?}"))
        })?;
        let (_, device_name) = bracketed_name(device_tail)?;
        let card_index = card_index.parse::<u16>().map_err(|_| {
            PlaybackDiscoveryError::Malformed(format!("invalid card index in {line:?}"))
        })?;
        let device = device.parse::<u16>().map_err(|_| {
            PlaybackDiscoveryError::Malformed(format!("invalid device index in {line:?}"))
        })?;
        let base_identity = base_identity(sound_class, card_index);
        observations.push(AlsaPlaybackObservation {
            card_index,
            card_id: card_id.to_owned(),
            card_name: card_name.to_owned(),
            device,
            device_name: device_name.to_owned(),
            base_identity,
        });
    }
    observations.sort_by(|left, right| {
        (&left.base_identity, &left.card_id, left.device).cmp(&(
            &right.base_identity,
            &right.card_id,
            right.device,
        ))
    });
    Ok(observations)
}

fn bracketed_name(value: &str) -> Result<(&str, &str), PlaybackDiscoveryError> {
    let open = value
        .find('[')
        .ok_or_else(|| PlaybackDiscoveryError::Malformed(format!("missing '[' in {value:?}")))?;
    let close = value[open + 1..]
        .find(']')
        .map(|index| open + 1 + index)
        .ok_or_else(|| PlaybackDiscoveryError::Malformed(format!("missing ']' in {value:?}")))?;
    Ok((value[..open].trim(), value[open + 1..close].trim()))
}

fn base_identity(sound_class: &Path, card_index: u16) -> String {
    let device = sound_class.join(format!("card{card_index}/device"));
    std::fs::canonicalize(device)
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_owned()))
        .and_then(|name| name.to_str().map(str::to_owned))
        .unwrap_or_else(|| format!("alsa-card-{card_index}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_exact_playback_device_rows_without_opening() {
        let listing = "**** List of PLAYBACK Hardware Devices ****\ncard 0: PCH [HDA Intel PCH], device 0: ALC3253 Analog [ALC3253 Analog]\n  Subdevices: 1/1\ncard 0: PCH [HDA Intel PCH], device 3: HDMI 0 [HDMI 0]\n";
        let observations = parse_aplay_list(listing, Path::new("/nonexistent")).unwrap();
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].card_id, "PCH");
        assert_eq!(observations[0].device, 0);
        assert_eq!(observations[0].device_name, "ALC3253 Analog");
        assert_eq!(observations[0].base_identity, "alsa-card-0");
        assert_eq!(observations[1].device, 3);
    }

    #[test]
    fn malformed_device_rows_fail_closed() {
        let error = parse_aplay_list("card nonsense", Path::new("/nonexistent")).unwrap_err();
        assert!(matches!(error, PlaybackDiscoveryError::Malformed(_)));
    }

    #[test]
    fn no_playback_device_is_an_exact_empty_observation_set() {
        let observations = parse_aplay_list(
            "**** List of PLAYBACK Hardware Devices ****\n",
            Path::new("/nonexistent"),
        )
        .unwrap();
        assert!(observations.is_empty());
    }
}
