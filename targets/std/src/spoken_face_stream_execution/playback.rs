//! Exact spoken Face segments through one selected speaker resource.
//!
//! An installed Host can explicitly fan one converted PCM stream to both its
//! selected speaker and retained WAV artifact in the same Plan and Play.

use super::*;
use crate::hosted_audio::{
    ExplicitPlaybackAuthorization, HostedPlaybackSelection, PlaybackLifecycle, PlaybackReport,
};
use crate::spoken_face_mask::{SpokenBatchDelivery, SpokenBatchPlaybackReceipt};
use crate::RunControl;
use conduit_core::{CapabilityId, GearId, OfferGeneration, SignId};
use conduit_planner::PlacementChoice;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const MAXIMUM_CAPTURE_WAV_BYTES: u64 = 44 + 30 * 48_000 * 4;

/// At 25 source frames per block, 16,384 blocks cover the admitted 16,384 ms
/// even when every PCM block is full. A 3,072-block limit covers only 3.48 s.
/// The selected Host adapter holds at most two seconds of PCM under pressure.
/// Short utterances below its startup lead begin on input close.
pub fn spoken_playback_plot(language: &conduit_language::LanguageRequest) -> String {
    let language_request = crate::hosted_language::language_request_literal(language);
    format!("plot spoken_face_playback (\n >> segments: SpeakableText...|\n) {{\n voice: speech/synthesize-stream(language-request = {language_request}, maximum-output-bytes = 1323000, maximum-audio-millis = 30000, maximum-segments = 32)\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\", maximum-blocks = 32768, maximum-audio-millis = 30000)\n speaker: audio/play(maximum-blocks = 32768, maximum-audio-millis = 30000)\n segments >> voice.text\n voice.audio >> convert.audio\n convert.converted >> speaker.audio\n}}.\n")
}

/// The two effects are explicit siblings of one converted stream. Neither a
/// later synthesis nor a second Play can stand in for the listener's PCM.
pub fn spoken_playback_with_capture_plot(language: &conduit_language::LanguageRequest) -> String {
    let language_request = crate::hosted_language::language_request_literal(language);
    format!("plot spoken_face_playback (\n >> segments: SpeakableText...|\n) {{\n voice: speech/synthesize-stream(language-request = {language_request}, maximum-output-bytes = 1323000, maximum-audio-millis = 30000, maximum-segments = 32)\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\", maximum-blocks = 32768, maximum-audio-millis = 30000)\n speaker: audio/play(maximum-blocks = 32768, maximum-audio-millis = 30000)\n artifact: audio/play(maximum-blocks = 32768, maximum-audio-millis = 30000)\n segments >> voice.text\n voice.audio >> convert.audio\n convert.converted >> speaker.audio\n convert.converted >> artifact.audio\n}}.\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpokenPlaybackOutcome {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenPlaybackExecution {
    pub outcome: SpokenPlaybackOutcome,
    pub face_id: String,
    pub face_revision: u64,
    pub source_show_id: String,
    pub stream_identity: String,
    pub source_segments_sha256: String,
    pub provider_sha256: String,
    pub host_id: String,
    pub boot_id: String,
    pub offer_generation: OfferGeneration,
    pub selected_resource_pool_id: String,
    pub authority_grant_id: String,
    pub source_document_id: String,
    pub checked_plot_id: String,
    pub playback_plan_id: String,
    pub playback_play_id: String,
    pub playback: PlaybackReport,
    /// Present only when a second, explicitly planned sink completed in this
    /// very same Play. Its PCM is the converted stream sent to the speaker.
    pub same_play_capture: Option<SamePlayCapture>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamePlayCapture {
    pub wav_path: PathBuf,
    pub wav_sha256: String,
    pub wav_bytes: u64,
    pub pcm_sha256: String,
    pub pcm_bytes: u32,
    pub pcm_blocks: u16,
}

impl SpokenPlaybackExecution {
    /// Turn a terminal selected-output effect into the reader's distinct
    /// playback acknowledgement. Failures cannot be promoted to success.
    pub fn delivery(&self) -> SpokenBatchDelivery {
        match self.outcome {
            SpokenPlaybackOutcome::Completed => {
                SpokenBatchDelivery::Played(SpokenBatchPlaybackReceipt {
                    stream_identity: self.stream_identity.clone(),
                    source_show_id: self.source_show_id.clone(),
                    source_segments_sha256: self.source_segments_sha256.clone(),
                    speech_plan_id: self.playback_plan_id.clone(),
                    speech_play_id: self.playback_play_id.clone(),
                    provider_sha256: self.provider_sha256.clone(),
                    playback: self.playback.clone(),
                })
            }
            SpokenPlaybackOutcome::Cancelled => SpokenBatchDelivery::Cancelled,
            SpokenPlaybackOutcome::Failed => SpokenBatchDelivery::Failed(
                "selected speaker Play did not drain successfully".into(),
            ),
        }
    }
}

/// The caller supplies a fresh exact Host Boot, one explicitly chosen ALSA
/// observation, and independent playback authorization. A current discovery
/// observation is rechecked at the first PCM Host Call by the selected back.
#[allow(clippy::too_many_arguments)]
pub fn execute_real_spoken_batch_to_selected_playback(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
    discovery: EspeakDiscovery,
    language: &conduit_language::LanguageRequest,
    config: StdHostConfig,
    selection: HostedPlaybackSelection,
    authorization: &ExplicitPlaybackAuthorization,
    control: &RunControl,
) -> Result<SpokenPlaybackExecution, SpokenStreamExecutionRefusal> {
    validate_spoken_source(face, source_show, batch)?;
    let selection = selection.with_bounded_speech_queue();
    if selection.boot_id != config.boot_id || selection.offer_generation != config.offer_generation
    {
        return Err(SpokenStreamExecutionRefusal::Plan(
            "selected playback observation does not match Host Boot".into(),
        ));
    }
    let provider_sha256 = discovery.provider_sha256.clone();
    let adapter = discovery
        .initialize(
            config.host_id.clone(),
            config.boot_id.clone(),
            config.offer_generation,
            "grant/spoken-face-playback-speech".into(),
            Duration::from_secs(30),
        )
        .map_err(|error| SpokenStreamExecutionRefusal::Plan(error.to_string()))?;
    let mut host = StdHost::new_with_playback(
        config.clone(),
        StdHostComposition::minimal().with_text(),
        selection.clone(),
    )
    .map_err(SpokenStreamExecutionRefusal::Plan)?;
    host.attach_espeak_speech_for_selected_playback(adapter)
        .map_err(SpokenStreamExecutionRefusal::Plan)?;
    run_selected_spoken_playback(
        face,
        source_show,
        batch,
        language,
        &provider_sha256,
        config,
        selection,
        authorization,
        control,
        &mut host,
        false,
    )
}

/// Run a spoken batch on the actual already-attached Host. The caller retains
/// that Host across batches and must keep its selected speaker and initialized
/// speech provider attached. This entrance never creates a second Host with
/// copied Host/Boot identifiers and does not mint a spoken Mask Show.
#[allow(clippy::too_many_arguments)]
pub fn execute_spoken_batch_on_attached_host(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
    language: &conduit_language::LanguageRequest,
    selection: &HostedPlaybackSelection,
    authorization: &ExplicitPlaybackAuthorization,
    control: &RunControl,
    host: &mut StdHost,
) -> Result<SpokenPlaybackExecution, SpokenStreamExecutionRefusal> {
    let offered = host.advertisement();
    let config = StdHostConfig {
        host_id: offered.host_id.clone(),
        boot_id: offered.boot_id.clone(),
        offer_generation: offered.offer_generation,
    };
    let provider = host.speech_synthesis.as_ref().ok_or_else(|| {
        SpokenStreamExecutionRefusal::Plan("Host has no initialized speech provider".into())
    })?;
    provider
        .validate_host(&config.host_id, &config.boot_id, config.offer_generation)
        .map_err(|error| SpokenStreamExecutionRefusal::Plan(error.to_string()))?;
    let provider_sha256 = provider.provider_sha256().to_owned();
    run_selected_spoken_playback(
        face,
        source_show,
        batch,
        language,
        &provider_sha256,
        config,
        selection.clone(),
        authorization,
        control,
        host,
        false,
    )
}

/// The installed owner's selected speaker and artifact are separately granted
/// and exactly placed, then driven by one atomic fan-out in one Play.
#[allow(clippy::too_many_arguments)]
pub fn execute_spoken_batch_on_attached_host_with_capture(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
    language: &conduit_language::LanguageRequest,
    selection: &HostedPlaybackSelection,
    authorization: &ExplicitPlaybackAuthorization,
    control: &RunControl,
    host: &mut StdHost,
) -> Result<SpokenPlaybackExecution, SpokenStreamExecutionRefusal> {
    let offered = host.advertisement();
    let config = StdHostConfig {
        host_id: offered.host_id.clone(),
        boot_id: offered.boot_id.clone(),
        offer_generation: offered.offer_generation,
    };
    let provider = host.speech_synthesis.as_ref().ok_or_else(|| {
        SpokenStreamExecutionRefusal::Plan("Host has no initialized speech provider".into())
    })?;
    provider
        .validate_host(&config.host_id, &config.boot_id, config.offer_generation)
        .map_err(|error| SpokenStreamExecutionRefusal::Plan(error.to_string()))?;
    let provider_sha256 = provider.provider_sha256().to_owned();
    run_selected_spoken_playback(
        face,
        source_show,
        batch,
        language,
        &provider_sha256,
        config,
        selection.clone(),
        authorization,
        control,
        host,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_selected_spoken_playback(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
    language: &conduit_language::LanguageRequest,
    provider_sha256: &str,
    config: StdHostConfig,
    selection: HostedPlaybackSelection,
    authorization: &ExplicitPlaybackAuthorization,
    control: &RunControl,
    host: &mut StdHost,
    capture: bool,
) -> Result<SpokenPlaybackExecution, SpokenStreamExecutionRefusal> {
    validate_spoken_source(face, source_show, batch)?;
    if host.advertisement().host_id != config.host_id
        || host.advertisement().boot_id != config.boot_id
        || host.advertisement().offer_generation != config.offer_generation
        || host.playback.as_ref() != Some(&selection)
    {
        return Err(SpokenStreamExecutionRefusal::Plan(
            "spoken playback Host differs from selected resource".into(),
        ));
    }
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
    let checked = check_syntax_document(
        &parse_syntax_document(&if capture {
            spoken_playback_with_capture_plot(language)
        } else {
            spoken_playback_plot(language)
        }),
        &startup,
    )
    .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "spoken_face_playback", &profiles)
            .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;

    let hosts = [host.advertisement().clone()];
    let mut placements = conduit_planner::default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| SpokenStreamExecutionRefusal::Plan(format!("{error:?}")))?;
    let speaker = authoring
        .expanded
        .gears
        .iter()
        .find(|gear| gear.gear_id == GearId::from("spoken_face_playback/speaker"))
        .ok_or_else(|| SpokenStreamExecutionRefusal::Plan("Plot omitted speaker gear".into()))?;
    placements.by_gear.insert(
        speaker.gear_id.clone(),
        PlacementChoice {
            host_id: config.host_id.clone(),
            capability_id: CapabilityId::from("audio-play-alsa-hw"),
        },
    );
    if capture {
        if !host.wav_artifact.as_ref().is_some_and(|artifact| {
            artifact.boot_id == config.boot_id
                && artifact.offer_generation == config.offer_generation
                && artifact.is_unpublished()
        }) {
            return Err(SpokenStreamExecutionRefusal::Plan(
                "selected retained WAV artifact capacity is unavailable".into(),
            ));
        }
        let artifact = authoring
            .expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id == GearId::from("spoken_face_playback/artifact"))
            .ok_or_else(|| {
                SpokenStreamExecutionRefusal::Plan("Plot omitted artifact gear".into())
            })?;
        placements.by_gear.insert(
            artifact.gear_id.clone(),
            PlacementChoice {
                host_id: config.host_id.clone(),
                capability_id: CapabilityId::from("audio-write-wav-artifact"),
            },
        );
    }
    let mut grants = vec![host
        .playback_authority_grant(authorization.grant_id())
        .map_err(SpokenStreamExecutionRefusal::Plan)?];
    if host.speech_synthesis.is_some() {
        grants.push(
            host.streaming_speech_authority_grant()
                .map_err(SpokenStreamExecutionRefusal::Plan)?,
        );
    }
    if capture {
        grants.push(
            host.wav_artifact_authority_grant("grant/spoken-face-same-play-wav")
                .map_err(SpokenStreamExecutionRefusal::Plan)?,
        );
    }
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
    if !plan.realization_backs.is_empty() {
        return Err(SpokenStreamExecutionRefusal::Plan(
            "selected playback sealing cannot discard Plot back provenance".into(),
        ));
    }
    let selected_pool = selection.pool_id();
    let plan = conduit_planner::seal_exact_plan_with_selected_realizations(
        plan,
        &hosts,
        &[selection.realization_advertisement(config.host_id.clone())],
        &[selection.resource_observation(
            config.host_id.clone(),
            SignId::from("sign/spoken-face-playback-resource-ready"),
        )],
    )
    .map_err(|error| SpokenStreamExecutionRefusal::Plan(format!("{error:?}")))?;
    let [fragment] = plan.fragments.as_slice() else {
        return Err(SpokenStreamExecutionRefusal::Plan(
            "spoken playback Plan was not one local fragment".into(),
        ));
    };
    let speaker = fragment
        .placements
        .iter()
        .find(|placement| placement.gear_id == GearId::from("spoken_face_playback/speaker"))
        .ok_or_else(|| SpokenStreamExecutionRefusal::Plan("Plan omitted audio/play".into()))?;
    if speaker.implementation_id.as_str() != conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
        || speaker.resources.len() != 1
        || speaker.resources[0].pool_id != selected_pool
        || speaker.authority.len() != 1
        || speaker.authority[0].grant_id.as_str() != authorization.grant_id()
    {
        return Err(SpokenStreamExecutionRefusal::Plan(
            "Plan did not bind the selected speaker resource and grant".into(),
        ));
    }
    if capture {
        let artifact = fragment
            .placements
            .iter()
            .find(|placement| placement.gear_id == GearId::from("spoken_face_playback/artifact"))
            .ok_or_else(|| {
                SpokenStreamExecutionRefusal::Plan("Plan omitted WAV artifact".into())
            })?;
        if artifact.implementation_id.as_str()
            != conduit_std_offers::AUDIO_WAV_ARTIFACT_IMPLEMENTATION
            || artifact.resources.len() != 1
            || artifact.resources[0].pool_id.as_str() != "std/audio/wav-artifact"
            || artifact.authority.len() != 1
            || artifact.authority[0].grant_id.as_str() != "grant/spoken-face-same-play-wav"
        {
            return Err(SpokenStreamExecutionRefusal::Plan(
                "Plan did not bind the selected artifact resource and grant".into(),
            ));
        }
    }
    let inputs = batch
        .encoded_inputs()
        .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;
    struct NoOutput;
    impl ExternalForeOutputAdapter for NoOutput {
        fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
            Err("spoken playback Plot has no external output".into())
        }
    }
    struct NoTimer;
    impl TimerAdapter for NoTimer {
        fn wait(&mut self, _: Duration) {}
    }
    let report = host
        .run_external_plot_sequence_controlled_to(
            fragment.clone(),
            &inputs,
            &mut NoOutput,
            &mut Vec::new(),
            &mut NoTimer,
            control,
        )
        .map_err(|detail| SpokenStreamExecutionRefusal::PlaybackPlay {
            source_show_id: batch.source_show_id.clone(),
            source_segments_sha256: batch.source_segments_sha256.clone(),
            plan_id: plan.plan_id.as_str().into(),
            selected_resource_pool_id: selected_pool.as_str().into(),
            detail,
        })?;
    let terminal = report.observations.last().map(|item| &item.kind);
    let kernel = report
        .kernel
        .ok_or(SpokenStreamExecutionRefusal::IncompletePlay)?;
    let [playback] = kernel.playback.as_slice() else {
        return Err(SpokenStreamExecutionRefusal::IncompletePlay);
    };
    let outcome = match terminal {
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed,
        }) if playback.lifecycle == PlaybackLifecycle::StoppedClosed
            && playback.metrics.blocks_committed > 0
            && playback.metrics.underruns == 0 =>
        {
            SpokenPlaybackOutcome::Completed
        }
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. },
        }) => SpokenPlaybackOutcome::Cancelled,
        Some(ObservationKind::PlanTerminal { .. }) => SpokenPlaybackOutcome::Failed,
        _ => return Err(SpokenStreamExecutionRefusal::IncompletePlay),
    };
    let same_play_capture = if capture && outcome == SpokenPlaybackOutcome::Completed {
        let [wav] = kernel.wav_artifacts.as_slice() else {
            return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
        };
        let locator = wav
            .locator
            .as_ref()
            .ok_or(SpokenStreamExecutionRefusal::IncompleteArtifact)?;
        let path = PathBuf::from(locator);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| SpokenStreamExecutionRefusal::IncompleteArtifact)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAXIMUM_CAPTURE_WAV_BYTES
        {
            return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW);
        let mut file = options
            .open(&path)
            .map_err(|_| SpokenStreamExecutionRefusal::IncompleteArtifact)?;
        let opened = file
            .metadata()
            .map_err(|_| SpokenStreamExecutionRefusal::IncompleteArtifact)?;
        if !opened.is_file() || opened.len() != metadata.len() {
            return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
        }
        #[cfg(unix)]
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.by_ref()
            .take(MAXIMUM_CAPTURE_WAV_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| SpokenStreamExecutionRefusal::IncompleteArtifact)?;
        if !wav.completed
            || wav.pcm_bytes == 0
            || wav.blocks == 0
            || bytes.len() != wav.pcm_bytes as usize + 44
            || bytes.get(0..4) != Some(b"RIFF")
            || bytes.get(8..12) != Some(b"WAVE")
            || u64::from(wav.frames) != playback.metrics.frames_committed
            || u32::from(wav.blocks) != playback.metrics.blocks_committed
            || wav.pcm_sha256.as_deref() != Some(playback.committed_pcm_sha256.as_str())
            || wav.pcm_sha256.as_deref()
                != Some(format!("{:x}", Sha256::digest(&bytes[44..])).as_str())
        {
            return Err(SpokenStreamExecutionRefusal::IncompleteArtifact);
        }
        Some(SamePlayCapture {
            wav_path: path,
            wav_sha256: format!("{:x}", Sha256::digest(&bytes)),
            wav_bytes: bytes.len() as u64,
            pcm_sha256: format!("{:x}", Sha256::digest(&bytes[44..])),
            pcm_bytes: wav.pcm_bytes,
            pcm_blocks: wav.blocks,
        })
    } else {
        None
    };
    Ok(SpokenPlaybackExecution {
        outcome,
        face_id: batch.face_id.clone(),
        face_revision: batch.face_revision,
        source_show_id: batch.source_show_id.clone(),
        stream_identity: batch.stream_identity.clone(),
        source_segments_sha256: batch.source_segments_sha256.clone(),
        provider_sha256: provider_sha256.into(),
        host_id: config.host_id.as_str().into(),
        boot_id: config.boot_id.as_str().into(),
        offer_generation: config.offer_generation,
        selected_resource_pool_id: selected_pool.as_str().into(),
        authority_grant_id: authorization.grant_id().into(),
        source_document_id: checked.source_document_id.as_str().into(),
        checked_plot_id: authoring.expanded.checked_plot_id.as_str().into(),
        playback_plan_id: plan.plan_id.as_str().into(),
        playback_play_id: kernel.active_play_id.as_str().into(),
        playback: playback.clone(),
        same_play_capture,
    })
}
