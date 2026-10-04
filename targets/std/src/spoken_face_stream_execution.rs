//! Planned hosted synthesis of exact mechanical spoken Face segments.
//!
//! The installed std Host feeds committed Tongues segments sequentially through
//! one exact external Fore Flow. It never renumbers or flattens them.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use conduit_core::{kind_id, BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{ManifestationLifecycle, MaskShow, Presentation};
use sha2::{Digest, Sha256};

use crate::{
    hosted_speech_synthesis::EspeakDiscovery,
    hosted_wav_artifact::WavArtifactSelection,
    spoken_face_mask::{SpokenBatch, SpokenBatchAudioReceipt},
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
    TimerAdapter,
};

#[path = "spoken_face_stream_execution/playback.rs"]
mod playback;
pub use playback::{
    execute_real_spoken_batch_to_selected_playback, execute_spoken_batch_on_attached_host,
    SpokenPlaybackExecution, SpokenPlaybackOutcome, SPOKEN_PLAYBACK_PLOT,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpokenStreamExecutionRefusal {
    StaleFace,
    StaleShow,
    InvalidBatch,
    ExistingOutput,
    InvalidOutput,
    Check(String),
    Plan(String),
    Play(String),
    PlaybackPlay {
        source_show_id: String,
        source_segments_sha256: String,
        plan_id: String,
        selected_resource_pool_id: String,
        detail: String,
    },
    IncompletePlay,
    IncompleteArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenStreamExecution {
    pub wav_path: PathBuf,
    pub receipt: SpokenBatchAudioReceipt,
    pub host_id: String,
    pub boot_id: String,
    pub source_document_id: String,
    pub checked_plot_id: String,
}

/// The portable Plot uses the same real synthesis, conversion, and artifact
/// backs as `prove-speech --stream`. The Front is already committed Tongues
/// text; adding `speech/commit-generated-text` here would commit it twice.
pub const SPOKEN_SEGMENT_PLOT: &str = "plot spoken_face_stream (\n >> segments: SpeakableText...|\n) {\n voice: speech/synthesize-stream(maximum-output-bytes = 1323000, maximum-audio-millis = 30000, maximum-segments = 32)\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\", maximum-blocks = 32768, maximum-audio-millis = 30000)\n artifact: audio/play(maximum-blocks = 32768, maximum-audio-millis = 30000)\n segments >> voice.text\n voice.audio >> convert.audio\n convert.converted >> artifact.audio\n}.\n";

pub fn execute_real_spoken_batch(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
    discovery: EspeakDiscovery,
    wav_path: &Path,
) -> Result<SpokenStreamExecution, SpokenStreamExecutionRefusal> {
    validate_batch_for_installed_fore(face, source_show, batch, wav_path)?;
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias(
            "SpeakableText",
            kind_id(conduit_tongues::SPEAKABLE_TEXT_VALUE_KIND),
        )
        .map_err(SpokenStreamExecutionRefusal::Check)?;
    let mut profiles = ProfileCatalog::new();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)
        .map_err(SpokenStreamExecutionRefusal::Check)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)
        .map_err(SpokenStreamExecutionRefusal::Check)?;
    let checked = check_syntax_document(&parse_syntax_document(SPOKEN_SEGMENT_PLOT), &startup)
        .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;
    let authoring = expand_canonical_plot_for_authoring(&checked, "spoken_face_stream", &profiles)
        .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;

    let fresh = StdHost::new();
    let offered = fresh.advertisement();
    let config = StdHostConfig {
        host_id: offered.host_id.clone(),
        boot_id: offered.boot_id.clone(),
        offer_generation: offered.offer_generation,
    };
    let provider_sha256 = discovery.provider_sha256.clone();
    let adapter = discovery
        .initialize(
            config.host_id.clone(),
            config.boot_id.clone(),
            config.offer_generation,
            "grant/spoken-face-speech".into(),
            Duration::from_secs(30),
        )
        .map_err(|error| SpokenStreamExecutionRefusal::Plan(error.to_string()))?;
    let artifact =
        WavArtifactSelection::new(wav_path, config.boot_id.clone(), config.offer_generation)
            .map_err(SpokenStreamExecutionRefusal::Plan)?;
    let mut host = StdHost::new_with_composition(config, StdHostComposition::minimal().with_text());
    host.attach_espeak_speech_and_wav_artifact(adapter, artifact)
        .map_err(SpokenStreamExecutionRefusal::Plan)?;
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| SpokenStreamExecutionRefusal::Plan(format!("{error:?}")))?;
    let grants = [
        host.streaming_speech_authority_grant()
            .map_err(SpokenStreamExecutionRefusal::Plan)?,
        host.wav_artifact_authority_grant("grant/spoken-face-wav")
            .map_err(SpokenStreamExecutionRefusal::Plan)?,
    ];
    let boundary = BTreeMap::from([(
        conduit_planner::ForeBoundaryKey {
            direction: conduit_core::PortDirection::Input,
            front_port_id: conduit_core::port_id("segments"),
            track: conduit_core::ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 2048,
        },
    )]);
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 2048,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary,
    )
    .map_err(|error| SpokenStreamExecutionRefusal::Plan(format!("{error:?}")))?;
    let [fragment] = plan.fragments.as_slice() else {
        return Err(SpokenStreamExecutionRefusal::Plan(
            "speech Plan was not one local fragment".into(),
        ));
    };
    if !fragment.placements.iter().any(|placement| {
        placement.implementation_id.as_str() == conduit_std_offers::ESPEAK_STREAM_IMPLEMENTATION
    }) {
        return Err(SpokenStreamExecutionRefusal::Plan(
            "speech Plan omitted eSpeak streaming back".into(),
        ));
    }
    let inputs = batch
        .encoded_inputs()
        .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;
    struct NoOutput;
    impl ExternalForeOutputAdapter for NoOutput {
        fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
            Err("speech Plot has no external output".into())
        }
    }
    struct NoTimer;
    impl TimerAdapter for NoTimer {
        fn wait(&mut self, _: Duration) {}
    }
    let report = host
        .run_external_plot_sequence_to(
            fragment.clone(),
            &inputs,
            &mut NoOutput,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .map_err(SpokenStreamExecutionRefusal::Play)?;
    if !matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ) {
        return Err(SpokenStreamExecutionRefusal::IncompletePlay);
    }
    let kernel = report
        .kernel
        .ok_or(SpokenStreamExecutionRefusal::IncompletePlay)?;
    let [wav] = kernel.wav_artifacts.as_slice() else {
        return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
    };
    if !wav.completed || wav.pcm_bytes == 0 || wav.blocks == 0 {
        return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
    }
    let bytes = fs::read(wav_path).map_err(|_| SpokenStreamExecutionRefusal::IncompleteArtifact)?;
    if bytes.len() != wav.pcm_bytes as usize + 44
        || bytes.get(..4) != Some(b"RIFF")
        || bytes.get(8..12) != Some(b"WAVE")
    {
        return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
    }
    let receipt = SpokenBatchAudioReceipt {
        stream_identity: batch.stream_identity.clone(),
        source_show_id: batch.source_show_id.clone(),
        source_segments_sha256: batch.source_segments_sha256.clone(),
        speech_plan_id: plan.plan_id.as_str().into(),
        speech_play_id: kernel.active_play_id.as_str().into(),
        provider_sha256,
        wav_sha256: format!("{:x}", Sha256::digest(&bytes)),
        wav_bytes: bytes.len() as u64,
        pcm_bytes: wav.pcm_bytes,
        pcm_blocks: wav.blocks,
    };
    Ok(SpokenStreamExecution {
        wav_path: wav_path.to_path_buf(),
        receipt,
        host_id: hosts[0].host_id.as_str().into(),
        boot_id: hosts[0].boot_id.as_str().into(),
        source_document_id: checked.source_document_id.as_str().into(),
        checked_plot_id: authoring.expanded.checked_plot_id.as_str().into(),
    })
}

/// Pure preflight before provider discovery, planning, or artifact creation.
/// The batch owns the exact finite committed sequence for one Fore Flow.
pub fn validate_batch_for_installed_fore(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
    wav_path: &Path,
) -> Result<(), SpokenStreamExecutionRefusal> {
    validate_spoken_source(face, source_show, batch)?;
    if wav_path.exists() {
        return Err(SpokenStreamExecutionRefusal::ExistingOutput);
    }
    if !wav_path.parent().is_some_and(Path::is_dir) {
        return Err(SpokenStreamExecutionRefusal::InvalidOutput);
    }
    Ok(())
}

fn validate_spoken_source(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
) -> Result<(), SpokenStreamExecutionRefusal> {
    if face.identity.as_str() != batch.face_id || face.revision != batch.face_revision {
        return Err(SpokenStreamExecutionRefusal::StaleFace);
    }
    if source_show.show_id.as_str() != batch.source_show_id
        || source_show.show.lifecycle != ManifestationLifecycle::Available
        || source_show.validate(face).is_err()
    {
        return Err(SpokenStreamExecutionRefusal::StaleShow);
    }
    batch
        .validate(face, source_show)
        .map_err(|_| SpokenStreamExecutionRefusal::InvalidBatch)?;
    Ok(())
}

#[cfg(test)]
#[path = "spoken_face_stream_execution/tests.rs"]
mod tests;
