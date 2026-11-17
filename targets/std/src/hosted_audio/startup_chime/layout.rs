//! Borrowed producer sum for preparation and both finite process invocations.
use super::*;
use crate::hosted_process::layout::process_layout;
use std::mem::size_of;
pub(crate) fn preparation_bytes(selection: &HostedPlaybackSelection) -> Result<usize, String> {
    let endpoint = super::super::playback_identity::endpoint_len(selection, "std/alsa-output/")
        .ok_or("speaker endpoint overflow")?;
    let pool = super::super::playback_identity::endpoint_len(selection, "std/audio/alsa/")
        .ok_or("speaker pool overflow")?;
    let target =
        super::super::playback_identity::target_len(selection).ok_or("speaker target overflow")?;
    let observation = &selection.observation;
    let clone = observation
        .card_id
        .len()
        .checked_add(observation.card_name.len())
        .and_then(|n| n.checked_add(observation.device_name.len()))
        .and_then(|n| n.checked_add(observation.base_identity.len()))
        .and_then(|n| n.checked_add(selection.boot_id.as_str().len()))
        .and_then(|n| n.checked_add(selection.source_base_id.as_ref().map_or(0, String::len)))
        .ok_or("speaker clone overflow")?;
    let mechanism = if selection.source_base_id.is_some() {
        "audio/speaker"
    } else {
        "std.base/alsa-playback@1"
    };
    let mut total = 0usize;
    let mut add = |value: usize| -> Result<(), String> {
        total = total
            .checked_add(value)
            .ok_or("speaker workspace overflow")?;
        Ok(())
    };
    // Exact checked provider binding temporaries, including endpoint+digest and
    // independent instance/base/implementation/mechanism and pool strings.
    for value in [
        64,
        endpoint,
        endpoint
            .checked_add(65)
            .ok_or("speaker instance overflow")?,
        selection
            .source_base_id
            .as_ref()
            .map_or(endpoint, String::len),
        conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION.len(),
        mechanism.len(),
        pool,
        clone,
    ] {
        add(value)?;
    }
    add((ARGUMENTS.len() + 1)
        .checked_mul(size_of::<OsString>())
        .ok_or("speaker argv overflow")?)?;
    for value in ARGUMENTS {
        add(value.len())?;
    }
    add(target)?;
    add(7)?; // temporary fixed environment strings
    add("--playback".len() + "-l".len())?; // borrowed array's owned OsString payloads
    let metadata = process_layout(
        "/usr/bin/aplay".len(),
        ["--playback".len(), "-l".len()],
        64 * 1024,
        4096,
    )?;
    let playback = process_layout(
        "/usr/bin/aplay".len(),
        ARGUMENTS
            .iter()
            .map(|value| value.len())
            .chain(std::iter::once(target)),
        0,
        4096,
    )?;
    for value in [
        metadata.preparation_bytes,
        metadata.spawn_bytes,
        playback.preparation_bytes,
        playback.spawn_bytes,
    ] {
        add(value)?;
    }
    let mut digits = 1usize;
    let mut index = observation.card_index;
    while index >= 10 {
        index /= 10;
        digits += 1;
    }
    add("/sys/class/sound/card/device".len() + digits + 1)?;
    add("alsa-card-".len() + digits)?;
    #[cfg(test)]
    {
        add(clone)?;
    }
    // Static failure prefixes, including validation/preparation refusal.
    add("startup chime differs from exact selected Plan/speaker/PCM".len())?;
    Ok(total)
}
