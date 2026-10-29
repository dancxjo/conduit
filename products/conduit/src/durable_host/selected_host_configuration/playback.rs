//! Reobserve an explicit speaker without opening a PCM stream or granting output authority.
use super::super::{bounded_read, digest};
use conduit_host_make::BaseInitialization;
use conduit_std_host::{
    hosted_audio::{discover_alsa_playback, AlsaPlaybackObservation, HostedPlaybackSelection},
    StdHost,
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    executable_sha256: String,
    card_id: String,
    device: u16,
    base_identity: String,
}
fn parameters(initialization: &BaseInitialization) -> Result<Parameters, String> {
    let catalog = conduit_workspace_make::catalog();
    let contract = catalog
        .initialization_contracts
        .get(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION)
        .ok_or("speaker initialization contract unavailable")?;
    initialization.checked_values(contract)?;
    let p: Parameters =
        serde_json::from_value(serde_json::to_value(initialization).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if p.executable_sha256.len() != 64
        || !p
            .executable_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("speaker requires an exact lowercase executable SHA-256".into());
    }
    Ok(p)
}
pub(super) fn validate(initialization: &BaseInitialization) -> Result<(), String> {
    parameters(initialization).map(|_| ())
}
fn select(
    p: &Parameters,
    observations: Vec<AlsaPlaybackObservation>,
) -> Result<AlsaPlaybackObservation, String> {
    let mut matches = observations.into_iter().filter(|v| {
        v.card_id == p.card_id && v.device == p.device && v.base_identity == p.base_identity
    });
    let selected = matches
        .next()
        .ok_or("selected speaker endpoint identity is missing")?;
    if matches.next().is_some() {
        return Err("selected speaker endpoint identity is ambiguous".into());
    }
    Ok(selected)
}
pub(super) fn initialize(
    initialization: &BaseInitialization,
    host: &StdHost,
) -> Result<HostedPlaybackSelection, String> {
    let p = parameters(initialization)?;
    // This implementation owns its executable path; source cannot substitute another program.
    let observed = digest(&bounded_read(
        std::path::Path::new("/usr/bin/aplay"),
        8 * 1024 * 1024,
    )?);
    if observed.strip_prefix("sha256:") != Some(p.executable_sha256.as_str()) {
        return Err("selected speaker executable identity changed".into());
    }
    let observation = select(&p, discover_alsa_playback().map_err(|e| e.to_string())?)?;
    let after = digest(&bounded_read(
        std::path::Path::new("/usr/bin/aplay"),
        8 * 1024 * 1024,
    )?);
    if after != observed {
        return Err("speaker executable changed during discovery".into());
    }
    let offer = host.advertisement();
    HostedPlaybackSelection::from_observation(
        observation,
        offer.boot_id.clone(),
        offer.offer_generation,
    )
    .with_executable_sha256(p.executable_sha256)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parameters_fixture() -> Parameters {
        Parameters {
            executable_sha256: "a".repeat(64),
            card_id: "Headphones".into(),
            device: 0,
            base_identity: "bcm2835-audio".into(),
        }
    }
    #[test]
    fn exact_speaker_selection_refuses_missing_drifted_and_ambiguous_endpoints() {
        let p = parameters_fixture();
        let o = AlsaPlaybackObservation {
            card_index: 0,
            card_id: p.card_id.clone(),
            card_name: "Fixture".into(),
            device: 0,
            device_name: "Fixture".into(),
            base_identity: p.base_identity.clone(),
        };
        assert_eq!(select(&p, vec![o.clone()]).unwrap(), o);
        assert!(select(&p, vec![]).is_err());
        let mut changed = o.clone();
        changed.base_identity = "replacement".into();
        assert!(select(&p, vec![changed]).is_err());
        assert!(select(&p, vec![o.clone(), o]).is_err());
    }
}
