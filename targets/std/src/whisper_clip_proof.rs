//! Supported proof-only entrance for one supplied PCM clip through ordinary plan/Play.

use crate::{
    hosted_speech_recognition::WhisperSpeechAdapter, StdHost, StdHostComposition, StdHostConfig,
    ThreadTimer,
};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhisperClipPlanPlayReceipt {
    pub plan_id: String,
    pub play_id: String,
    pub implementation_id: String,
    pub audio_sha256: String,
    pub text_sha256: Option<String>,
    pub text_bytes: u16,
    pub diagnostic_bytes: u16,
}

pub fn run(
    config: StdHostConfig,
    composition: StdHostComposition,
    adapter: WhisperSpeechAdapter,
    clip: Vec<u8>,
) -> Result<WhisperClipPlanPlayReceipt, String> {
    conduit_audio::decode_pcm_clip(&clip)
        .map_err(|error| format!("decode admitted proof PCM clip: {error:?}"))?;
    let mut host = StdHost::new_with_whisper_clip_speech_recognition(config, composition, adapter)?;
    host.attach_proof_pcm_clip_source(clip)?;

    let mut catalog = conduit_form::ProfileCatalog::new();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut catalog)?;
    crate::installed_std::test_local_model_io::install_house_source_catalog(
        &mut startup,
        &mut catalog,
    );
    crate::installed_std::test_local_model_io::install_house_recognition_sink_catalog(
        &mut startup,
        &mut catalog,
    );
    let source = format!(
        "form whisper_recorded_clip_proof {{\n audio: {}\n recognize: speech/recognize-clip\n sink: {}\n audio.value > recognize.clip\n recognize.result > sink.value\n}}\n",
        crate::installed_std::test_local_model_io::HOUSE_AUDIO_CLIP_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_RECOGNITION_SINK_KIND,
    );
    let form = conduit_form::parse(&source, &catalog)
        .map_err(|error| format!("parse Whisper clip proof Form: {error}"))?;
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements)
        .map_err(|error| format!("place Whisper clip proof: {error}"))?;
    let plan = conduit_planner::plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_audio::MAXIMUM_PCM_CLIP_BYTES
                .max(conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES as usize)
                as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| format!("plan Whisper clip proof: {error}"))?;
    let plan_id = plan.plan_id.clone();
    let fragment = plan
        .fragments
        .into_iter()
        .next()
        .ok_or_else(|| "Whisper clip proof Plan has no fragment".to_string())?;
    let report = host.run_fragment_to(fragment, &mut Vec::new(), &mut ThreadTimer)?;
    if !matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ) {
        return Err("Whisper clip proof Play did not complete".into());
    }
    let recognition = report
        .speech_recognition
        .into_iter()
        .next()
        .ok_or_else(|| "Whisper clip proof omitted recognition receipt".to_string())?;
    Ok(WhisperClipPlanPlayReceipt {
        plan_id: plan_id.as_str().to_string(),
        play_id: recognition.active_play_id.as_str().to_string(),
        implementation_id: recognition.implementation_id.as_str().to_string(),
        audio_sha256: recognition
            .audio_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        text_sha256: recognition.text_sha256,
        text_bytes: recognition.text_bytes,
        diagnostic_bytes: recognition.diagnostic_bytes,
    })
}
