//! IMAGE-selected playback initialization. Metadata inspection never grants playback.
use super::{AlsaPlaybackObservation, HostedPlaybackSelection};
use conduit_core::{BaseProviderEntry, BootId, OfferGeneration};
use conduit_host_make::CheckedHostConfiguration;
use serde::Deserialize;

pub(crate) struct InitialPlayback {
    pub(crate) selection: HostedPlaybackSelection,
    pub(crate) entry: BaseProviderEntry,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    executable_sha256: String,
    card_id: String,
    device: u16,
    base_identity: String,
}

pub(crate) fn prepare(
    checked: &CheckedHostConfiguration,
    boot: BootId,
    generation: OfferGeneration,
) -> Result<Option<InitialPlayback>, String> {
    prepare_with(
        checked,
        boot,
        generation,
        |expected| {
            super::executable_identity::verify_file(
                std::path::Path::new("/usr/bin/aplay"),
                expected,
            )
            .map_err(|error| format!("selected playback executable: {error:?}"))
        },
        || super::discover_alsa_playback().map_err(|error| error.to_string()),
    )
}

fn prepare_with(
    checked: &CheckedHostConfiguration,
    boot: BootId,
    generation: OfferGeneration,
    mut verify: impl FnMut(&[u8; 32]) -> Result<(), String>,
    discover: impl FnOnce() -> Result<Vec<AlsaPlaybackObservation>, String>,
) -> Result<Option<InitialPlayback>, String> {
    let mut declarations = checked
        .configuration()
        .bases
        .iter()
        .filter(|base| base.kind == "audio/speaker");
    let Some(base) = declarations.next() else {
        return Ok(None);
    };
    if declarations.next().is_some()
        || !checked
            .resolved_bases()
            .iter()
            .any(|(kind, implementation)| {
                kind == "audio/speaker"
                    && implementation == conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
            })
    {
        return Err("selected playback declaration is ambiguous or unsupported".into());
    }
    let initialization = base
        .initialization
        .as_ref()
        .ok_or("selected playback initialization is required")?;
    let intent: Intent = serde_json::from_value(
        serde_json::to_value(&initialization.values).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("selected playback initialization: {e}"))?;
    let digest = parse_digest(&intent.executable_sha256)?;
    // Verify the independently selected executable before invoking discovery.
    verify(&digest)?;
    let observations = discover()?;
    // Discovery must not silently replace the executable whose identity was selected.
    verify(&digest)?;
    let mut matches = observations.into_iter().filter(|observation| {
        observation.card_id == intent.card_id
            && observation.device == intent.device
            && observation.base_identity == intent.base_identity
    });
    let observation = matches.next().ok_or("selected playback device is absent")?;
    if matches.next().is_some() {
        return Err("selected playback observation is ambiguous".into());
    }
    let selection = HostedPlaybackSelection::from_observation(observation, boot, generation)
        .with_executable_sha256(intent.executable_sha256)?;
    let mut entry = selection
        .checked_base_entry()
        .ok_or("checked playback provider identity is absent")?;
    // The installed IMAGE names the public construction Base, not the legacy
    // standalone attachment family's namespace. All provider/capability facts
    // still come from the exact initialized selection above.
    entry.mechanism_family = "audio/speaker".into();
    Ok(Some(InitialPlayback { selection, entry }))
}

fn parse_digest(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("playback executable requires an exact lowercase SHA-256".into());
    }
    let mut digest = [0; 32];
    for (out, pair) in digest.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        let nibble = |byte: u8| {
            if byte.is_ascii_digit() {
                byte - b'0'
            } else {
                byte - b'a' + 10
            }
        };
        *out = nibble(pair[0]) * 16 + nibble(pair[1]);
    }
    Ok(digest)
}

#[cfg(test)]
#[path = "image_initialization_tests.rs"]
mod tests;
