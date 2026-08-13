use super::{HostedPlaybackSelection, PlaybackReport};
use crate::{StdHost, StdHostConfig, ThreadTimer};
use conduit_core::{ActivePlayId, CharacteristicId, ConnectionBase, HostId, PlanId, SignId};
use std::collections::BTreeMap;
use std::io::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitPlaybackAuthorization {
    grant_id: String,
}

impl ExplicitPlaybackAuthorization {
    pub fn new(grant_id: &str) -> Result<Self, String> {
        if grant_id.is_empty() || grant_id == "default" {
            return Err("playback authorization grant identity must be explicit".into());
        }
        Ok(Self {
            grant_id: grant_id.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackProofReceipt {
    pub host_id: HostId,
    pub boot_id: conduit_core::BootId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub authority_grant_id: String,
    pub playback: PlaybackReport,
}

/// Runs the repository-development specimen through the production installed
/// kernel and exact real playback operation. The proof source is advertised
/// only by the special proof Host constructor, never by an ordinary std Host.
pub fn run_playback_proof<W: Write>(
    selection: HostedPlaybackSelection,
    authorization: ExplicitPlaybackAuthorization,
    output: &mut W,
) -> Result<PlaybackProofReceipt, String> {
    let host_id = HostId::from("std-hosted-audio-proof");
    let config = StdHostConfig {
        host_id: host_id.clone(),
        boot_id: selection.boot_id.clone(),
        offer_generation: selection.offer_generation,
    };
    let realization = selection.realization_advertisement(host_id.clone());
    let observation = selection.resource_observation(
        host_id.clone(),
        SignId::from("sign/hosted-audio-proof-resource-ready"),
    );
    let selected_pool = selection.pool_id();
    let selected_target = selection.alsa_target();
    let mut host = StdHost::new_with_playback_proof(config, selection)?;
    let form = conduit_form::parse(
        "form 0\n\nhosted_audio_proof {\n source: conduit.proof/pcm-specimen-source\n output: audio/play\n source.audio -> output.audio\n}\n",
        &crate::installed_std::playback_proof_catalog(),
    )
    .map_err(|error| format!("parse hosted audio proof: {error}"))?;
    let grant = host.playback_authority_grant(&authorization.grant_id)?;
    let advertisements = [host.advertisement().clone()];
    let plan = conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        &form,
        conduit_planner::SelectedRealizationPlanning {
            hosts: &advertisements,
            bases: &[ConnectionBase::Local],
            requirements: &BTreeMap::new(),
            advertisements: &[realization],
            observations: &[observation],
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            authority_grants: &[grant],
        },
    )
    .map_err(|error| format!("plan hosted audio proof: {error:?}"))?;
    let fragment = plan
        .fragments
        .first()
        .cloned()
        .ok_or_else(|| "hosted audio proof has no local fragment".to_string())?;
    let playback_placement = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::AUDIO_PLAY_KIND)
        .ok_or_else(|| "hosted audio proof has no audio/play placement".to_string())?;
    for required in [
        conduit_std_catalog::AUDIO_SAMPLE_REPRESENTATION_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_SAMPLE_RATE_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_CHANNEL_LAYOUT_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_PERIOD_FRAMES_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_BUFFER_FRAMES_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_MAXIMUM_BLOCKS_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_SOURCE_CLOCK_ID_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_DEVICE_CLOCK_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_PLAYBACK_RESOURCE_CHARACTERISTIC,
        conduit_std_catalog::AUDIO_TIMING_CLASS_CHARACTERISTIC,
    ] {
        let id = CharacteristicId::from(required);
        if !playback_placement
            .realization_characteristics
            .iter()
            .any(|fact| fact.definition.characteristic_id == id)
        {
            return Err(format!(
                "audio Plan omitted required exact fact '{required}'"
            ));
        }
    }
    writeln!(
        output,
        "audio-proof selected host={} boot={} generation={} resource={} target={} authority={} profile={} plan={}",
        host_id.as_str(),
        advertisements[0].boot_id.as_str(),
        advertisements[0].offer_generation.0,
        selected_pool.as_str(),
        selected_target,
        authorization.grant_id,
        playback_placement.execution_profile_id.as_str(),
        fragment.plan_id.as_str(),
    )
    .map_err(|error| error.to_string())?;
    let report = host.run_fragment_to(fragment.clone(), output, &mut ThreadTimer)?;
    let kernel = report
        .kernel
        .ok_or_else(|| "hosted audio proof did not use the production kernel".to_string())?;
    let playback = kernel
        .playback
        .into_iter()
        .next()
        .ok_or_else(|| "hosted audio proof retained no playback evidence".to_string())?;
    writeln!(
        output,
        "audio-proof result lifecycle={:?} backend={} blocks={} frames={} period_frames={} buffer_frames={} underruns={} first_commit_us={} write_min_us={} write_max_us={} timing_class={} clock_correlation={} controlled_staging_bytes={} external_buffer_class={}",
        playback.lifecycle,
        playback.backend,
        playback.metrics.blocks_committed,
        playback.metrics.frames_committed,
        playback.metrics.period_frames,
        playback.metrics.buffer_frames,
        playback.metrics.underruns,
        playback.metrics.first_commit_micros.unwrap_or(0),
        playback.metrics.minimum_write_micros.unwrap_or(0),
        playback.metrics.maximum_write_micros.unwrap_or(0),
        playback.timing_class,
        playback.clock_correlation,
        playback.controlled_staging_bytes,
        playback.external_buffer_class,
    )
    .map_err(|error| error.to_string())?;
    Ok(PlaybackProofReceipt {
        host_id,
        boot_id: advertisements[0].boot_id.clone(),
        plan_id: fragment.plan_id,
        active_play_id: kernel.active_play_id,
        authority_grant_id: authorization.grant_id,
        playback,
    })
}
