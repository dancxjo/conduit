//! Explicit microphone capture through Whisper in one ordinary Plan/Play.

use crate::hosted_microphone::{AlsaMicrophoneAdapter, MicrophoneCaptureReceipt};
use crate::hosted_speech_recognition::WhisperSpeechAdapter;
use crate::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MicrophoneWhisperProofReceipt {
    pub plan_id: String,
    pub play_id: String,
    pub microphone_implementation_id: String,
    pub microphone_executable_sha256: String,
    pub microphone_base_identity: String,
    pub microphone_card_id: String,
    pub microphone_device: u16,
    pub capture_milliseconds: u32,
    pub raw_pcm_sha256: String,
    pub raw_pcm_bytes: u32,
    pub microphone_diagnostic_bytes: u16,
    pub whisper_implementation_id: String,
    pub recognized_text_sha256: Option<String>,
    pub recognized_text_bytes: u16,
}

pub fn run(
    config: StdHostConfig,
    composition: StdHostComposition,
    microphone: AlsaMicrophoneAdapter,
    whisper: WhisperSpeechAdapter,
) -> Result<MicrophoneWhisperProofReceipt, Box<dyn std::error::Error>> {
    let mut host = StdHost::new_with_microphone(config, composition, microphone)?;
    host.attach_whisper_clip_recognizer(whisper)?;
    let mut profiles = ProfileCatalog::new();
    let mut startup = StartupCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_microphone_clip_catalogs(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profiles)?;
    let source = "form microphone-whisper-proof {\n microphone: media/capture-microphone-clip\n recognize: speech/recognize-clip\n text: speech/recognition-to-text\n show: presentation/text\n \"capture\" > microphone.request\n microphone.clip > recognize.clip\n recognize.result > text.result\n text.text > show.text\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &startup).map_err(|error| {
            format!(
                "microphone Whisper Form check: {} {}",
                error.code, error.message
            )
        })?;
    let expanded =
        conduit_form::expand_canonical_form(&checked, "microphone-whisper-proof", &profiles)
            .map_err(|error| {
                format!(
                    "microphone Whisper expansion: {} {}",
                    error.code, error.message
                )
            })?;
    let mut connection_limits = BTreeMap::new();
    for connection in &expanded.connections {
        let byte_capacity = match connection.source_port_id.as_str() {
            "clip" => conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
            "result" => conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
            _ => continue,
        };
        connection_limits.insert(
            (
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ),
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity,
            },
        );
    }
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &advertisements)?;
    let grant = host.microphone_authority_grant("grant/microphone-whisper-proof")?;
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
    )?;
    let plan_id = plan.plan_id.as_str().to_string();
    let fragment = plan
        .fragments
        .into_iter()
        .next()
        .ok_or("microphone Whisper Plan has no fragment")?;
    let report = host.run_fragment_to(fragment, &mut Vec::new(), &mut ThreadTimer)?;
    if !matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ) {
        return Err("microphone Whisper Play did not complete".into());
    }
    let microphone = exactly_one(report.microphone, "microphone")?;
    let recognition = exactly_one(report.speech_recognition, "Whisper recognition")?;
    Ok(receipt(plan_id, microphone, recognition))
}

fn receipt(
    plan_id: String,
    microphone: MicrophoneCaptureReceipt,
    recognition: crate::SpeechRecognitionExecutionReceipt,
) -> MicrophoneWhisperProofReceipt {
    MicrophoneWhisperProofReceipt {
        plan_id,
        play_id: recognition.active_play_id.as_str().to_string(),
        microphone_implementation_id: conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION.into(),
        microphone_executable_sha256: microphone.executable_sha256,
        microphone_base_identity: microphone.base_identity,
        microphone_card_id: microphone.card_id,
        microphone_device: microphone.device,
        capture_milliseconds: microphone.capture_milliseconds,
        raw_pcm_sha256: microphone.raw_pcm_sha256,
        raw_pcm_bytes: microphone.raw_pcm_bytes,
        microphone_diagnostic_bytes: microphone.diagnostic_bytes,
        whisper_implementation_id: recognition.implementation_id.as_str().to_string(),
        recognized_text_sha256: recognition.text_sha256,
        recognized_text_bytes: recognition.text_bytes,
    }
}

fn exactly_one<T>(mut values: Vec<T>, name: &str) -> Result<T, Box<dyn std::error::Error>> {
    if values.len() != 1 {
        return Err(format!(
            "microphone Whisper Play retained {} {name} receipts",
            values.len()
        )
        .into());
    }
    Ok(values.remove(0))
}
