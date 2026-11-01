//! A bounded ALSA verbose setup prefix confirms the user-facing PCM profile
//! before any frame escapes. Plug slave formats are separate provider facts.
//! Wire field spelling: alsa-lib src/pcm/pcm.c snd_pcm_dump_hw_setup;
//! alsa-utils aplay/aplay.c dumps setup before capture data transfer.
use super::MicrophoneFailure;

pub(super) fn confirmed(bytes: &[u8]) -> Result<bool, MicrophoneFailure> {
    let mut stream = None;
    let mut access = None;
    let mut format = None;
    let mut channels = None;
    let mut rate = None;
    let mut exact = None;
    let mut buffer = None;
    let mut period = None;
    // Only newline-terminated lines are meaningful; a field split over writes
    // cannot become a false match or force a premature malformed-text error.
    let Some(end) = bytes.iter().rposition(|byte| *byte == b'\n') else {
        return Ok(false);
    };
    for line in bytes[..end].split(|byte| *byte == b'\n') {
        if line.starts_with(b"Recording raw data ") {
            return Ok([
                stream, access, format, channels, rate, exact, buffer, period,
            ]
            .iter()
            .all(|value| *value == Some(true)));
        }
        let Some(split) = line.iter().position(|byte| *byte == b':') else {
            continue;
        };
        let key = line[..split].trim_ascii();
        let value = line[split + 1..].trim_ascii();
        match key {
            b"stream" if stream.is_none() => stream = Some(value == b"CAPTURE"),
            b"access" if access.is_none() => access = Some(value == b"RW_INTERLEAVED"),
            b"format" if format.is_none() => format = Some(value == b"S16_LE"),
            b"channels" if channels.is_none() => channels = Some(value == b"1"),
            b"rate" if rate.is_none() => rate = Some(value == b"16000"),
            b"exact rate" if exact.is_none() => exact = Some(exact_rate(value)),
            b"buffer_size" if buffer.is_none() => buffer = Some(bounded_number(value, 8192)),
            b"period_size" if period.is_none() => period = Some(bounded_number(value, 2048)),
            _ => {}
        }
        // The first setup is the application's stream, not a converted slave.
        if [
            stream, access, format, channels, rate, exact, buffer, period,
        ]
        .contains(&Some(false))
        {
            return Err(MicrophoneFailure::InexactFormat);
        }
    }
    Ok(false)
}
fn bounded_number(bytes: &[u8], maximum: u64) -> bool {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|value| value > 0 && value <= maximum)
}
fn exact_rate(bytes: &[u8]) -> bool {
    let Some(begin) = bytes.iter().position(|byte| *byte == b'(') else {
        return false;
    };
    let Some(value) = bytes[begin + 1..].strip_suffix(b")") else {
        return false;
    };
    let Ok(value) = std::str::from_utf8(value) else {
        return false;
    };
    let Some((numerator, denominator)) = value.split_once('/') else {
        return false;
    };
    let (Ok(numerator), Ok(denominator)) = (numerator.parse::<u64>(), denominator.parse::<u64>())
    else {
        return false;
    };
    denominator != 0 && denominator.checked_mul(16000) == Some(numerator)
}
#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE:&[u8]=b"  stream: CAPTURE\n  access: RW_INTERLEAVED\n  format: S16_LE\n  channels: 1\n  rate: 16000\n  exact rate: 16000 (16000/1)\n  buffer_size: 1024\n  period_size: 256\nRecording raw data stdout : Signed 16 bit Little Endian, Rate 16000 Hz, Mono\n";
    #[test]
    fn split_lines_and_converted_slave_cannot_change_application_profile() {
        for end in 0..PROFILE.len() {
            assert!(!confirmed(&PROFILE[..end]).unwrap());
        }
        assert!(confirmed(PROFILE).unwrap());
        let mut slave = PROFILE.to_vec();
        slave.extend_from_slice(b"Slave:\n format: S32_LE\n channels: 2\n rate: 48000\n");
        assert!(confirmed(&slave).unwrap());
    }
    #[test]
    fn wrong_profile_and_overlarge_buffers_refuse_before_delivery() {
        for (from, to) in [
            ("S16_LE", "S32_LE"),
            ("rate: 16000", "rate: 48000"),
            ("channels: 1", "channels: 2"),
            ("buffer_size: 1024", "buffer_size: 100000"),
            ("16000/1", "16001/1"),
        ] {
            let profile = std::str::from_utf8(PROFILE).unwrap().replace(from, to);
            assert_eq!(
                confirmed(profile.as_bytes()),
                Err(MicrophoneFailure::InexactFormat)
            );
        }
    }
}
