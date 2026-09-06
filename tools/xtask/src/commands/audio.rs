use crate::cli::GlobalOpts;
use crate::output::{RepositoryOutput, MAXIMUM_OUTPUT_ITEMS};
use conduit_core::{BootId, OfferGeneration};
use conduit_std_host::hosted_audio::{
    discover_alsa_playback, run_playback_proof, ExplicitPlaybackAuthorization,
    HostedPlaybackSelection,
};
use serde::Serialize;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIO_LIST_SCHEMA: &str = "conduit.tools/xtask/hosted-audio-list@1";
const AUDIO_PROOF_SCHEMA: &str = "conduit.tools/xtask/hosted-audio-proof@1";

#[derive(Serialize)]
struct AudioListReport<'a> {
    schema: &'static str,
    dry_run: bool,
    effects_performed: bool,
    observations: Vec<AudioObservationReport<'a>>,
}

#[derive(Serialize)]
struct AudioObservationReport<'a> {
    card_id: &'a str,
    card_index: u16,
    device: u16,
    base_identity: &'a str,
    card_name: &'a str,
    device_name: &'a str,
}

#[derive(Serialize)]
struct AudioProofReport<'a> {
    schema: &'static str,
    dry_run: bool,
    effects_performed: bool,
    card_id: &'a str,
    device: u16,
    authority: &'static str,
    resource_pool_id: Option<&'a str>,
    alsa_target: Option<&'a str>,
    host_id: Option<&'a str>,
    boot_id: Option<&'a str>,
    plan_id: Option<&'a str>,
    play_id: Option<&'a str>,
}

pub fn list(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let output = RepositoryOutput::from_opts(opts);
    if output.dry_run() {
        output.emit_json(&AudioListReport {
            schema: AUDIO_LIST_SCHEMA,
            dry_run: true,
            effects_performed: false,
            observations: Vec::new(),
        })?;
        output.emit_human(|writer| {
            writeln!(
                writer,
                "dry-run hosted audio list: metadata discovery not performed"
            )
        })?;
        return Ok(());
    }
    let observations = discover_alsa_playback()?;
    if observations.len() > MAXIMUM_OUTPUT_ITEMS {
        return Err("hosted audio observation output capacity exceeded".into());
    }
    if observations.is_empty() {
        return Err("no ALSA playback resources are currently observed".into());
    }
    output.emit_json(&AudioListReport {
        schema: AUDIO_LIST_SCHEMA,
        dry_run: false,
        effects_performed: false,
        observations: observations
            .iter()
            .map(|observation| AudioObservationReport {
                card_id: &observation.card_id,
                card_index: observation.card_index,
                device: observation.device,
                base_identity: &observation.base_identity,
                card_name: &observation.card_name,
                device_name: &observation.device_name,
            })
            .collect(),
    })?;
    output.emit_human(|writer| {
        writeln!(
            writer,
            "fresh hosted playback observations (no PCM device opened):"
        )?;
        for observation in &observations {
            writeln!(
                writer,
                "card-id={} card-index={} device={} base={} card-name={:?} device-name={:?}",
                observation.card_id,
                observation.card_index,
                observation.device,
                observation.base_identity,
                observation.card_name,
                observation.device_name,
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

pub fn prove(
    opts: &GlobalOpts,
    card_id: &str,
    device: u16,
    authorize_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = RepositoryOutput::from_opts(opts);
    if !authorize_output {
        return Err("hosted playback proof requires --authorize-output".into());
    }
    if output.dry_run() {
        output.emit_json(&AudioProofReport {
            schema: AUDIO_PROOF_SCHEMA,
            dry_run: true,
            effects_performed: false,
            card_id,
            device,
            authority: "explicit-not-exercised",
            resource_pool_id: None,
            alsa_target: None,
            host_id: None,
            boot_id: None,
            plan_id: None,
            play_id: None,
        })?;
        output.emit_human(|writer| writeln!(writer,
            "dry-run audio-proof card-id={} device={} authority=explicit-not-exercised device-opened=false",
            card_id, device
        ))?;
        return Ok(());
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
    let authorization =
        ExplicitPlaybackAuthorization::new("grant/xtask-explicit-hosted-audio-play")?;
    if matches!(output.mode(), crate::output::OutputMode::Human) {
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
    } else {
        let receipt = run_playback_proof(selection, authorization, &mut std::io::sink())?;
        let pool_id = receipt.playback.resource_pool_id.as_str();
        let target = receipt.playback.alsa_target.as_str();
        output.emit_json(&AudioProofReport {
            schema: AUDIO_PROOF_SCHEMA,
            dry_run: false,
            effects_performed: true,
            card_id,
            device,
            authority: "explicit-exercised",
            resource_pool_id: Some(pool_id),
            alsa_target: Some(target),
            host_id: Some(receipt.host_id.as_str()),
            boot_id: Some(receipt.boot_id.as_str()),
            plan_id: Some(receipt.plan_id.as_str()),
            play_id: Some(receipt.active_play_id.as_str()),
        })?;
    }
    Ok(())
}
