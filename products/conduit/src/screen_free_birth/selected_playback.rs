//! Explicitly selected installed speech and speaker output for one Face batch.
//! A produced voice stream is reported as played only after the selected
//! speaker Plan/Play drains and the reader accepts its exact receipt.

use conduit_core::HostAdvertisement;
use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::{
    hosted_audio::{
        discover_alsa_playback, ExplicitPlaybackAuthorization, HostedPlaybackSelection,
    },
    hosted_speech_synthesis::EspeakDiscovery,
    spoken_face_mask::SpokenBatch,
    spoken_face_stream_execution::{
        execute_real_spoken_batch_to_selected_playback, SpokenPlaybackExecution,
        SpokenStreamExecutionRefusal,
    },
    RunControl, StdHostConfig,
};

use crate::cli::BirthSpeechOptions;

#[derive(Clone)]
pub(super) struct SelectedPlayback {
    discovery: EspeakDiscovery,
    config: StdHostConfig,
    selection: HostedPlaybackSelection,
    authorization: ExplicitPlaybackAuthorization,
    voice: String,
}

impl SelectedPlayback {
    pub(super) fn prepare(
        options: &BirthSpeechOptions,
        advertisement: &HostAdvertisement,
    ) -> Result<Self, String> {
        if !options.speak {
            return Err("speaker playback was not selected".into());
        }
        let card = options
            .speaker_card
            .as_deref()
            .ok_or("speaker card is required")?;
        let device = options.speaker_device.ok_or("speaker device is required")?;
        let observations = discover_alsa_playback()
            .map_err(|error| format!("discover selected speaker: {error}"))?;
        let mut matches = observations
            .into_iter()
            .filter(|item| item.card_id == card && item.device == device);
        let observed = matches
            .next()
            .ok_or("selected speaker is absent from current ALSA discovery")?;
        if matches.next().is_some() {
            return Err("selected speaker observation is ambiguous".into());
        }
        let voice = options.speech_voice.as_deref().unwrap_or("en-us");
        let discovery = EspeakDiscovery::inspect(
            options
                .speech_executable
                .as_deref()
                .ok_or("speech executable is required")?,
            options
                .speech_data
                .as_deref()
                .ok_or("speech data is required")?,
            voice,
            &options.speech_engine,
        )
        .map_err(|error| format!("selected speech provider refused: {error:?}"))?;
        let config = StdHostConfig {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
        };
        let selection = HostedPlaybackSelection::from_observation(
            observed,
            config.boot_id.clone(),
            config.offer_generation,
        );
        let authorization = ExplicitPlaybackAuthorization::new(
            "grant/conduit/installed-screen-free-selected-speaker",
        )?;
        Ok(Self {
            discovery,
            config,
            selection,
            authorization,
            voice: voice.into(),
        })
    }

    pub(super) fn verify_host(&self, advertisement: &HostAdvertisement) -> Result<(), String> {
        if self.config.host_id != advertisement.host_id
            || self.config.boot_id != advertisement.boot_id
            || self.config.offer_generation != advertisement.offer_generation
        {
            return Err("selected speaker belongs to a stale installed Host Boot".into());
        }
        Ok(())
    }

    pub(super) fn play(
        &self,
        face: &Presentation,
        show: &MaskShow,
        batch: &SpokenBatch,
        control: &RunControl,
    ) -> Result<SpokenPlaybackExecution, SpokenStreamExecutionRefusal> {
        execute_real_spoken_batch_to_selected_playback(
            face,
            show,
            batch,
            self.discovery.clone(),
            self.config.clone(),
            self.selection.clone(),
            &self.authorization,
            control,
        )
    }

    pub(super) fn verify_receipt(
        &self,
        face: &Presentation,
        show: &MaskShow,
        batch: &SpokenBatch,
        result: &SpokenPlaybackExecution,
    ) -> Result<(), String> {
        if result.face_id != face.identity.as_str()
            || result.face_revision != face.revision
            || result.source_show_id != show.show_id.as_str()
            || result.stream_identity != batch.stream_identity
            || result.source_segments_sha256 != batch.source_segments_sha256
            || result.provider_sha256 != self.discovery.provider_sha256
            || result.host_id != self.config.host_id.as_str()
            || result.boot_id != self.config.boot_id.as_str()
            || result.offer_generation != self.config.offer_generation
            || result.selected_resource_pool_id != self.selection.pool_id().as_str()
            || result.authority_grant_id != self.authorization.grant_id()
        {
            return Err(
                "selected playback receipt differs from current Face, Show, or Host".into(),
            );
        }
        Ok(())
    }

    pub(super) fn receipt_json(&self, result: &SpokenPlaybackExecution) -> serde_json::Value {
        serde_json::json!({
            "schema": "conduit.body/spoken-face-playback@1",
            "outcome": format!("{:?}", result.outcome),
            "face_id": result.face_id,
            "face_revision": result.face_revision,
            "source_show_id": result.source_show_id,
            "stream_identity": result.stream_identity,
            "source_segments_sha256": result.source_segments_sha256,
            "voice": self.voice,
            "provider_sha256": result.provider_sha256,
            "host_id": result.host_id,
            "boot_id": result.boot_id,
            "offer_generation": result.offer_generation.0,
            "selected_resource_pool_id": result.selected_resource_pool_id,
            "authority_grant_id": result.authority_grant_id,
            "source_document_id": result.source_document_id,
            "checked_plot_id": result.checked_plot_id,
            "plan_id": result.playback_plan_id,
            "play_id": result.playback_play_id,
            "speaker_lifecycle": format!("{:?}", result.playback.lifecycle),
            "speaker_blocks_committed": result.playback.metrics.blocks_committed,
            "speaker_frames_committed": result.playback.metrics.frames_committed,
            "speaker_underruns": result.playback.metrics.underruns,
        })
    }
}
