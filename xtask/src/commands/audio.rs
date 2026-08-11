use crate::cli::GlobalOpts;
use conduit_core::{BootId, OfferGeneration};
use conduit_std_host::hosted_audio::{
    discover_alsa_playback, run_playback_proof, ExplicitPlaybackAuthorization,
    HostedPlaybackSelection,
};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn list(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    reject_structured_modes(opts)?;
    let observations = discover_alsa_playback()?;
    if observations.is_empty() {
        return Err("no ALSA playback resources are currently observed".into());
    }
    if !opts.quiet {
        println!("fresh hosted playback observations (no PCM device opened):");
        for observation in observations {
            println!(
                "card-id={} card-index={} device={} base={} card-name={:?} device-name={:?}",
                observation.card_id,
                observation.card_index,
                observation.device,
                observation.base_identity,
                observation.card_name,
                observation.device_name,
            );
        }
    }
    Ok(())
}

pub fn prove(
    opts: &GlobalOpts,
    card_id: &str,
    device: u16,
    authorize_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    reject_structured_modes(opts)?;
    if !authorize_output {
        return Err("hosted playback proof requires --authorize-output".into());
    }
    let matches = discover_alsa_playback()?
        .into_iter()
        .filter(|item| item.card_id == card_id && item.device == device)
        .collect::<Vec<_>>();
    let observation = match matches.as_slice() {
        [observation] => observation.clone(),
        [] => {
            return Err(format!(
                "selected ALSA playback resource card-id={card_id} device={device} is not freshly observed"
            )
            .into())
        }
        _ => return Err("selected ALSA playback identity is ambiguous".into()),
    };
    let boot_id = BootId::from(format!(
        "boot-hosted-audio-proof-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let selection =
        HostedPlaybackSelection::from_observation(observation, boot_id, OfferGeneration(1));
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "dry-run audio-proof resource={} target={} profile={} authority=explicit-not-exercised device-opened=false",
                selection.pool_id().as_str(),
                selection.alsa_target(),
                conduit_std_catalog::AUDIO_PLAY_ALSA_HW_PROFILE,
            );
        }
        return Ok(());
    }
    let authorization =
        ExplicitPlaybackAuthorization::new("grant/xtask-explicit-hosted-audio-play")?;
    if opts.quiet {
        run_playback_proof(selection, authorization, &mut std::io::sink())?;
    } else {
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        let receipt = run_playback_proof(selection, authorization, &mut output)?;
        writeln!(
            output,
            "audio-proof receipt host={} boot={} plan={} play={} authority={}",
            receipt.host_id.as_str(),
            receipt.boot_id.as_str(),
            receipt.plan_id.as_str(),
            receipt.active_play_id.as_str(),
            receipt.authority_grant_id,
        )?;
    }
    Ok(())
}

fn reject_structured_modes(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        return Err("--json is not yet supported by hosted audio commands".into());
    }
    Ok(())
}
