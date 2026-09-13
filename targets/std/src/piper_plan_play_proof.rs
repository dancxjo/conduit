//! Explicit proof entrance for real Piper through ordinary std Plan and Play.

use crate::hosted_speech::{PiperDiscovery, PiperLimits};
use crate::{StdHost, StdHostComposition, StdHostConfig, StdRunReport, ThreadTimer};
use conduit_core::{BaseImplementationId, BootId, HostId, ImplementationId, OfferGeneration};
use std::collections::BTreeMap;

pub struct PiperPlanPlayProof {
    pub run: StdRunReport,
    pub conversion_implementation_id: ImplementationId,
    pub output_frames: u64,
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
