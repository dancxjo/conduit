//! Explicit proof entrance for real Piper through ordinary std Plan and Play.

use crate::hosted_speech::{PiperDiscovery, PiperLimits};
use crate::{StdHost, StdHostComposition, StdHostConfig, StdRunReport, ThreadTimer};
use conduit_core::{
    BaseImplementationId, BootId, HostId, ImplementationId, OfferGeneration, SignId,
};
use std::collections::BTreeMap;

pub struct PiperPlanPlayProof {
    pub run: StdRunReport,
    pub conversion_implementation_id: ImplementationId,
    pub output_frames: u64,
}

pub struct PiperPlaybackPlanPlayProof {
    pub run: StdRunReport,
    pub conversion_implementation_id: ImplementationId,
    pub output_frames: u64,
    pub resource_pool_id: conduit_core::ResourcePoolId,
    pub alsa_target: String,
}

pub fn run_with_playback(
    discovery: PiperDiscovery,
    limits: PiperLimits,
    text: &str,
    playback: crate::hosted_audio::HostedPlaybackSelection,
    authority_grant_id: &str,
) -> Result<PiperPlaybackPlanPlayProof, Box<dyn std::error::Error>> {
    let adapter = discovery.initialize(limits)?;
    let host_id = HostId::from("std-piper-playback-proof-host");
    let config = StdHostConfig {
        host_id: host_id.clone(),
        boot_id: playback.boot_id.clone(),
        offer_generation: playback.offer_generation,
    };
    let realization = playback.realization_advertisement(host_id.clone());
    let observation = playback.resource_observation(
        host_id.clone(),
        SignId::from("sign/piper-playback-proof-resource-ready"),
    );
    let piper_realization = crate::hosted_speech::process_realization_advertisement(
        host_id.clone(),
        playback.boot_id.clone(),
        playback.offer_generation,
    );
    let piper_observation = crate::hosted_speech::process_resource_observation(
        host_id,
        playback.boot_id.clone(),
        playback.offer_generation,
        SignId::from("sign/piper-playback-proof-process-ready"),
    );
    let resource_pool_id = playback.pool_id();
    let alsa_target = playback.alsa_target();
    let mut host = StdHost::new_with_piper_speech_and_playback(
        config,
        StdHostComposition::reference(),
        adapter,
        playback,
    )?;
    let mut profile = conduit_form::ProfileCatalog::new();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profile)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)?;
    let quoted_text = serde_json::to_string(text)?;
    let source = format!(
        "form piper_playback_proof {{\n synthesize: speech/synthesize(maximum-output-bytes = {})\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\")\n output: audio/play\n {} > synthesize.text\n synthesize.audio > convert.audio\n convert.converted > output.audio\n}}\n",
        conduit_tongues::MAXIMUM_PCM_BYTES,
        quoted_text,
    );
    let form = conduit_form::parse(&source, &profile)?;
    let grant = host.playback_authority_grant(authority_grant_id)?;
    let advertisements = [host.advertisement().clone()];
    let plan = conduit_planner::plan_selected_realizations_with_characteristics_and_authority(
        &form,
        conduit_planner::SelectedRealizationPlanning {
            hosts: &advertisements,
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &[realization, piper_realization],
            observations: &[observation, piper_observation],
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
            authority_grants: &[grant],
        },
    )?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == host.advertisement().host_id)
        .ok_or("Piper playback proof plan has no local fragment")?;
    let conversion_implementation_id = fragment
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::AUDIO_CONVERT_PCM_PROFILE_KIND
                && placement.implementation_id.as_str()
                    == conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION
        })
        .map(|placement| placement.implementation_id.clone())
        .ok_or("Piper playback proof omitted the exact PCM conversion placement")?;
    let report = host.run_fragment_to(fragment, &mut std::io::sink(), &mut ThreadTimer)?;
    let receipt = report
        .speech_synthesis
        .first()
        .ok_or("Piper playback proof produced no exact synthesis receipt")?;
    let output_frames = u64::from(receipt.frames)
        .checked_mul(48_000)
        .and_then(|frames| frames.checked_add(22_049))
        .map(|frames| frames / 22_050)
        .ok_or("converted Piper frame count overflowed")?;
    if report
        .kernel
        .as_ref()
        .map_or(0, |kernel| kernel.playback.len())
        != 1
    {
        return Err("Piper playback proof retained no exact playback evidence".into());
    }
    Ok(PiperPlaybackPlanPlayProof {
        run: report,
        conversion_implementation_id,
        output_frames,
        resource_pool_id,
        alsa_target,
    })
}

pub fn run(
    discovery: PiperDiscovery,
    limits: PiperLimits,
    text: &str,
) -> Result<PiperPlanPlayProof, Box<dyn std::error::Error>> {
    let adapter = discovery.initialize(limits)?;
    let mut host = StdHost::new_with_piper_speech(
        StdHostConfig {
            host_id: HostId::from("std-piper-proof-host"),
            boot_id: BootId::from(crate::boot_identity::fresh_boot_id()),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        adapter,
    )?;
    let mut profile = conduit_form::ProfileCatalog::new();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profile)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)?;
    crate::installed_std::test_speech_sink::install_catalog(&mut profile);
    let quoted_text = serde_json::to_string(text)?;
    let source = format!(
        "form piper_proof {{\n synthesize: speech/synthesize(maximum-output-bytes = {})\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\")\n sink: {}\n {} > synthesize.text\n synthesize.audio > convert.audio\n convert.converted > sink.audio\n}}\n",
        conduit_tongues::MAXIMUM_PCM_BYTES,
        crate::installed_std::test_speech_sink::KIND,
        quoted_text,
    );
    let form = conduit_form::parse(&source, &profile)?;
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements)?;
    let plan = conduit_planner::plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == host.advertisement().host_id)
        .ok_or("Piper proof plan has no local fragment")?;
    let conversion_implementation_id = fragment
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::AUDIO_CONVERT_PCM_PROFILE_KIND
        })
        .filter(|placement| {
            placement.implementation_id.as_str()
                == conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION
        })
        .map(|placement| placement.implementation_id.clone())
        .ok_or("Piper proof plan omitted the exact PCM conversion placement")?;
    let report =
        host.run_fragment_to(fragment, &mut Vec::with_capacity(1_024), &mut ThreadTimer)?;
    if report.speech_synthesis.len() != 1 {
        return Err("Piper Plan/Play proof produced no exact synthesis receipt".into());
    }
    let source_frames = u64::from(report.speech_synthesis[0].frames);
    let output_frames = source_frames
        .checked_mul(48_000)
        .and_then(|frames| frames.checked_add(22_049))
        .map(|frames| frames / 22_050)
        .ok_or("converted Piper frame count overflowed")?;
    Ok(PiperPlanPlayProof {
        run: report,
        conversion_implementation_id,
        output_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosted_audio::{
        AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection, PlaybackLifecycle,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    #[test]
    fn initialized_piper_converts_into_one_selected_authorized_playback() {
        let root = std::env::temp_dir().join(format!(
            "conduit-piper-playback-plan-play-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let executable = root.join("piper-fixture");
        let model = root.join("voice.onnx");
        let config = root.join("voice.onnx.json");
        fs::write(
            &executable,
            "#!/bin/sh\ncat >/dev/null\ndd if=/dev/zero bs=1 count=678 2>/dev/null\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&model, b"bounded model fixture").unwrap();
        fs::write(&config, br#"{"audio":{"sample_rate":22050}}"#).unwrap();
        let discovery = PiperDiscovery::inspect(&executable, &model, &config, None).unwrap();
        let selection = HostedPlaybackSelection::deterministic_fake(
            AlsaPlaybackObservation {
                card_index: 0,
                card_id: "FIXTURE".into(),
                card_name: "Deterministic fixture".into(),
                device: 0,
                device_name: "Finite PCM sink".into(),
                base_identity: "fixture-base".into(),
            },
            BootId::from("piper-playback-test-boot"),
            OfferGeneration(1),
            FakePlaybackBehavior::Success,
        );
        let proof = run_with_playback(
            discovery,
            PiperLimits {
                maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
                maximum_frames: conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2),
                maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
                timeout: Duration::from_secs(2),
            },
            "Hi",
            selection,
            "grant/test-piper-playback",
        )
        .unwrap();
        assert_eq!(proof.run.speech_synthesis.len(), 1);
        assert_eq!(proof.output_frames, 738);
        let playback = &proof.run.kernel.as_ref().unwrap().playback[0];
        assert_eq!(playback.lifecycle, PlaybackLifecycle::StoppedClosed);
        assert_eq!(playback.metrics.blocks_committed, 14);
        assert_eq!(playback.metrics.frames_committed, 738);
        assert_eq!(playback.metrics.underruns, 0);
        fs::remove_dir_all(root).unwrap();
    }
}
