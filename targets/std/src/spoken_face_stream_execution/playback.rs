//! Exact spoken Face segments through one selected speaker resource.
//!
//! This is a distinct Play from WAV artifact production. A shared Face/Show
//! and segment digest correlate intent; they do not assert identical PCM.

use super::*;
use crate::hosted_audio::{
    ExplicitPlaybackAuthorization, HostedPlaybackSelection, PlaybackLifecycle, PlaybackReport,
};
use crate::spoken_face_mask::{SpokenBatchDelivery, SpokenBatchPlaybackReceipt};
use crate::RunControl;
use conduit_core::{OfferGeneration, SignId};

/// At 25 source frames per block, 16,384 blocks cover the admitted 16,384 ms
/// even when every PCM block is full. A 3,072-block limit covers only 3.48 s.
/// The selected Host adapter holds at most two seconds of PCM under pressure.
/// Short utterances below its startup lead begin on input close.
pub const SPOKEN_PLAYBACK_PLOT: &str = "plot spoken_face_playback (\n >> segments: SpeakableText...|\n) {\n voice: speech/synthesize-stream(maximum-output-bytes = 1323000, maximum-audio-millis = 30000, maximum-segments = 32)\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\", maximum-blocks = 16384, maximum-audio-millis = 16384)\n speaker: audio/play(maximum-blocks = 16384, maximum-audio-millis = 16384)\n segments >> voice.text\n voice.audio >> convert.audio\n convert.converted >> speaker.audio\n}.\n";

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
        &provider_sha256,
        config,
        selection,
        authorization,
        control,
        host,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_selected_spoken_playback(
    face: &Presentation,
    source_show: &MaskShow,
    batch: &SpokenBatch,
    provider_sha256: &str,
    config: StdHostConfig,
    selection: HostedPlaybackSelection,
    authorization: &ExplicitPlaybackAuthorization,
    control: &RunControl,
    mut host: StdHost,
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
    let checked = check_syntax_document(&parse_syntax_document(SPOKEN_PLAYBACK_PLOT), &startup)
        .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "spoken_face_playback", &profiles)
            .map_err(|error| SpokenStreamExecutionRefusal::Check(format!("{error:?}")))?;

    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| SpokenStreamExecutionRefusal::Plan(format!("{error:?}")))?;
    let mut grants = vec![host
        .playback_authority_grant(authorization.grant_id())
        .map_err(SpokenStreamExecutionRefusal::Plan)?];
    if host.speech_synthesis.is_some() {
        grants.push(
            host.streaming_speech_authority_grant()
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
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::AUDIO_PLAY_KIND)
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
    })
}
