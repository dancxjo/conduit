//! Resolve the default PCM once during preparation; Play keeps an exact device.
//! The finite probe opens then closes the default PCM with empty input. It never
//! submits samples. Unlike metadata enumeration, this may briefly acquire audio.
use super::{discover_alsa_playback, AlsaPlaybackObservation, PlaybackDiscoveryError};
use crate::hosted_process::{run_process, ProcessRequest, ProcessTerminal};
use std::{path::Path, time::Duration};

/// Resolve a supported ALSA default route to one enumerated hardware provider.
/// Virtual routes that do not expose a single hardware identity refuse. This
/// observation grants neither playback authority nor a resource reservation.
pub fn resolve_default_alsa_playback() -> Result<AlsaPlaybackObservation, PlaybackDiscoveryError> {
    let before = discover_alsa_playback()?;
    let limit = 16 * 1024;
    let report = run_process(
        &ProcessRequest {
            program: Path::new("/usr/bin/aplay"),
            arguments: &[
                "--device=default".into(),
                "--verbose".into(),
                "--file-type=raw".into(),
                "--format=S16_LE".into(),
                "--rate=48000".into(),
                "--channels=2".into(),
            ],
            environment: &[("LC_ALL".into(), "C".into())],
            stdin: &[],
            maximum_stdout_bytes: limit,
            maximum_stderr_bytes: limit,
            timeout: Duration::from_secs(5),
            require_process_group: true,
        },
        || false,
    )
    .map_err(|error| {
        PlaybackDiscoveryError::CommandFailed(format!("default PCM probe: {error:?}"))
    })?;
    if report.stdout.observed_bytes > limit as u64 || report.stderr.observed_bytes > limit as u64 {
        return Err(PlaybackDiscoveryError::OutputBoundExceeded);
    }
    match report.terminal {
        ProcessTerminal::Exited(status) if status.success() => {}
        ProcessTerminal::TimedOut => return Err(PlaybackDiscoveryError::TimedOut),
        other => {
            return Err(PlaybackDiscoveryError::CommandFailed(format!(
                "default PCM: {other:?}: {}",
                String::from_utf8_lossy(&report.stderr.retained)
            )))
        }
    }
    let mut output = report.stdout.retained;
    output.push(b'\n');
    output.extend_from_slice(&report.stderr.retained);
    let text = std::str::from_utf8(&output).map_err(|_| PlaybackDiscoveryError::InvalidUtf8)?;
    let selected = match_default_hardware(text, &before)?;
    if !discover_alsa_playback()?.contains(&selected) {
        return Err(PlaybackDiscoveryError::Malformed(
            "default provider changed during preparation".into(),
        ));
    }
    Ok(selected)
}

fn match_default_hardware(
    text: &str,
    observations: &[AlsaPlaybackObservation],
) -> Result<AlsaPlaybackObservation, PlaybackDiscoveryError> {
    let refuse = || {
        PlaybackDiscoveryError::Malformed(
            "default PCM does not resolve to one observed hardware provider".into(),
        )
    };
    let mut targets = text
        .lines()
        .filter_map(|line| line.split_once("Hardware PCM card ").map(|(_, tail)| tail));
    let target = targets.next().ok_or_else(refuse)?;
    if targets.next().is_some() {
        return Err(refuse());
    }
    let (card, tail) = target.split_once(" '").ok_or_else(refuse)?;
    let (name, tail) = tail.rsplit_once("' device ").ok_or_else(refuse)?;
    let (device, subdevice) = tail.split_once(" subdevice ").ok_or_else(refuse)?;
    let card: u16 = card.parse().map_err(|_| refuse())?;
    let device: u16 = device.parse().map_err(|_| refuse())?;
    subdevice.parse::<u16>().map_err(|_| refuse())?;
    let mut matches = observations
        .iter()
        .filter(|o| o.card_index == card && o.device == device && o.card_name == name);
    let found = matches.next().ok_or_else(refuse)?;
    if matches.next().is_some() {
        return Err(refuse());
    }
    Ok(found.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observed() -> AlsaPlaybackObservation {
        AlsaPlaybackObservation {
            card_index: 2,
            card_id: "Speaker".into(),
            card_name: "Fixture Speaker".into(),
            device: 1,
            device_name: "PCM".into(),
            base_identity: "stable-path".into(),
        }
    }
    #[test]
    fn resolves_only_exact_single_observed_default_provider() {
        let line =
            "Plug PCM: Hardware PCM card 2 'Fixture Speaker' device 1 subdevice 0\nIts setup is:";
        assert_eq!(
            match_default_hardware(line, &[observed()]).unwrap(),
            observed()
        );
        for text in [
            "default",
            "PipeWire PCM",
            "Hardware PCM card 0 'Fixture Speaker' device 1 subdevice 0",
            "Hardware PCM card 2 'Replacement' device 1 subdevice 0",
            "Hardware PCM card 2 'Fixture Speaker' device 2 subdevice 0",
        ] {
            assert!(
                match_default_hardware(text, &[observed()]).is_err(),
                "{text}"
            );
        }
        assert!(match_default_hardware(&format!("{line}\n{line}"), &[observed()]).is_err());
        assert!(match_default_hardware(line, &[]).is_err());
        assert!(match_default_hardware(line, &[observed(), observed()]).is_err());
    }
}
