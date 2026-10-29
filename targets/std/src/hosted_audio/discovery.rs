use crate::hosted_process::{run_process, ProcessError, ProcessRequest, ProcessTerminal};
use std::fmt;
use std::path::Path;
use std::time::Duration;

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
    TimedOut,
    OutputBoundExceeded,
    Malformed(String),
}

impl fmt::Display for PlaybackDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(formatter, "run aplay discovery: {error}"),
            Self::CommandFailed(detail) => write!(formatter, "aplay discovery failed: {detail}"),
            Self::InvalidUtf8 => formatter.write_str("aplay discovery was not UTF-8"),
            Self::TimedOut => formatter.write_str("aplay metadata discovery timed out"),
            Self::OutputBoundExceeded => {
                formatter.write_str("aplay metadata output bound exceeded")
            }
            Self::Malformed(detail) => write!(formatter, "malformed aplay discovery: {detail}"),
        }
    }
}

impl std::error::Error for PlaybackDiscoveryError {}

/// Enumerates playback resources without opening any PCM handle. `aplay -l`
/// reads ALSA control metadata only; actual usability is rechecked at Start.
pub fn discover_alsa_playback() -> Result<Vec<AlsaPlaybackObservation>, PlaybackDiscoveryError> {
    discover_at(
        Path::new("/usr/bin/aplay"),
        Path::new("/sys/class/sound"),
        Duration::from_secs(5),
    )
}

fn discover_at(
    program: &Path,
    sound_class: &Path,
    timeout: Duration,
) -> Result<Vec<AlsaPlaybackObservation>, PlaybackDiscoveryError> {
    const MAXIMUM_LISTING_BYTES: usize = 64 * 1024;
    const MAXIMUM_DIAGNOSTIC_BYTES: usize = 4096;
    let report = run_process(
        &ProcessRequest {
            program,
            arguments: &["--playback".into(), "-l".into()],
            environment: &[("LC_ALL".into(), "C".into())],
            stdin: &[],
            maximum_stdout_bytes: MAXIMUM_LISTING_BYTES,
            maximum_stderr_bytes: MAXIMUM_DIAGNOSTIC_BYTES,
            timeout,
            require_process_group: true,
        },
        || false,
    )
    .map_err(|error| match error {
        ProcessError::Launch(error) => PlaybackDiscoveryError::Command(error),
        other => {
            PlaybackDiscoveryError::CommandFailed(format!("metadata process refused: {other:?}"))
        }
    })?;
    if report.stdout.observed_bytes > MAXIMUM_LISTING_BYTES as u64
        || report.stderr.observed_bytes > MAXIMUM_DIAGNOSTIC_BYTES as u64
    {
        return Err(PlaybackDiscoveryError::OutputBoundExceeded);
    }
    match report.terminal {
        ProcessTerminal::Exited(status) if status.success() => {}
        ProcessTerminal::TimedOut => return Err(PlaybackDiscoveryError::TimedOut),
        other => {
            return Err(PlaybackDiscoveryError::CommandFailed(format!(
                "{other:?}: {}",
                String::from_utf8_lossy(&report.stderr.retained).trim()
            )))
        }
    }
    let stdout = std::str::from_utf8(&report.stdout.retained)
        .map_err(|_| PlaybackDiscoveryError::InvalidUtf8)?;
    parse_aplay_list(stdout, sound_class)
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

#[cfg(all(test, unix))]
mod process_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn metadata_uses_explicit_mode_and_refuses_overflow_and_timeout() {
        let root =
            std::env::temp_dir().join(format!("conduit-aplay-metadata-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let program = root.join("provider");
        let write = |script: &str| {
            std::fs::write(&program, script).unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        };
        write("#!/bin/sh\n[ \"$1\" = --playback ] && [ \"$2\" = -l ] && [ \"$LC_ALL\" = C ] || exit 90\nprintf 'card 999: Fixture [Fixture], device 0: Output [Output]\\n'\n");
        let observed =
            discover_at(&program, Path::new("/nonexistent"), Duration::from_secs(2)).unwrap();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].card_id, "Fixture");
        write("#!/bin/sh\nprintf '%070000d' 0\n");
        assert!(matches!(
            discover_at(&program, Path::new("/nonexistent"), Duration::from_secs(2)),
            Err(PlaybackDiscoveryError::OutputBoundExceeded)
        ));
        write("#!/bin/sh\nprintf '%05000d' 0 >&2\n");
        assert!(matches!(
            discover_at(&program, Path::new("/nonexistent"), Duration::from_secs(2)),
            Err(PlaybackDiscoveryError::OutputBoundExceeded)
        ));
        write("#!/bin/sh\nwhile :; do :; done\n");
        assert!(matches!(
            discover_at(
                &program,
                Path::new("/nonexistent"),
                Duration::from_millis(50)
            ),
            Err(PlaybackDiscoveryError::TimedOut)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
