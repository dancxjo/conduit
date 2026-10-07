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
    hosted_wav_artifact::{WavArtifactRetentionLimits, WavArtifactSelection},
    spoken_face_mask::SpokenBatch,
    spoken_face_stream_execution::{
        execute_spoken_batch_on_attached_host_with_capture, SpokenPlaybackExecution,
        SpokenPlaybackOutcome, SpokenStreamExecutionRefusal,
    },
    RunControl, StdHost, StdHostComposition, StdHostConfig,
};
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::cli::BirthSpeechOptions;

#[derive(Clone)]
pub(super) struct SelectedPlayback {
    discovery: EspeakDiscovery,
    config: StdHostConfig,
    selection: HostedPlaybackSelection,
    authorization: ExplicitPlaybackAuthorization,
    voice: String,
    host: Arc<Mutex<StdHost>>,
    remaining_plays: Arc<AtomicU16>,
}

// A screen-free invocation may make at most this many selected Plays. Reserve
// the entire range before the first one, so a later invocation of the same
// real Boot cannot reuse an ActivePlayId even after interruption or restart.
const PLAYS_PER_ENCOUNTER: u64 = 1024;
const SCREEN_FREE_SEQUENCE_START: u64 = 1 << 63;

fn reserve_play_sequence_range(state_dir: &Path, boot_id: &str) -> Result<u64, String> {
    let path = state_dir.join("screen-free-play-sequence");
    let pending = state_dir.join("screen-free-play-sequence.pending");
    let lock_path = state_dir.join("screen-free-play-sequence.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .map_err(|error| format!("open screen-free Play sequence lock: {error}"))?;
    lock.lock()
        .map_err(|error| format!("lock screen-free Play sequence: {error}"))?;
    // A crash before rename leaves this marker. Refuse instead of replaying
    // the old range; a human can inspect the retained state before recovery.
    if pending.exists() {
        return Err("screen-free Play sequence has an interrupted update".into());
    }
    let retained = match std::fs::read_to_string(&path) {
        Ok(retained) => Some(retained),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("read screen-free Play sequence: {error}")),
    };
    let first = if retained.is_none() {
        SCREEN_FREE_SEQUENCE_START
    } else {
        let retained = retained.as_deref().expect("checked retained state");
        let mut lines = retained.lines();
        let recorded_boot = lines
            .next()
            .ok_or("screen-free Play sequence has no Boot")?;
        let recorded_next = lines
            .next()
            .ok_or("screen-free Play sequence has no next value")?
            .parse::<u64>()
            .map_err(|_| "screen-free Play sequence is invalid")?;
        if lines.next().is_some() || recorded_next < SCREEN_FREE_SEQUENCE_START {
            return Err("screen-free Play sequence is invalid".into());
        }
        if recorded_boot == boot_id {
            recorded_next
        } else {
            SCREEN_FREE_SEQUENCE_START
        }
    };
    let next = first
        .checked_add(PLAYS_PER_ENCOUNTER)
        .ok_or("screen-free Play sequence exhausted")?;
    let mut update = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|error| format!("stage screen-free Play sequence: {error}"))?;
    write!(update, "{boot_id}\n{next}\n")
        .and_then(|_| update.sync_all())
        .and_then(|_| std::fs::rename(&pending, &path))
        .and_then(|_| std::fs::File::open(state_dir)?.sync_all())
        .map_err(|error| format!("retain screen-free Play sequence: {error}"))?;
    Ok(first)
}

impl SelectedPlayback {
    pub(super) fn prepare(
        options: &BirthSpeechOptions,
        advertisement: &HostAdvertisement,
        state_dir: &Path,
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
        let coverage = conduit_std_host::hosted_speech_synthesis::read_language_coverage(
            options
                .speech_language_coverage
                .as_ref()
                .ok_or("speech needs Language coverage")?,
        )
        .map_err(|error| error.to_string())?;
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
        let discovery = discovery
            .declare_language_coverage(coverage)
            .map_err(|error| format!("selected speech Language coverage refused: {error:?}"))?;
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
        // The installed owner's own selected speech has a separate finite
        // artifact pool. Birth's independently selected Plays must not consume
        // its capacity or alter its evidence directory.
        let artifact_root = state_dir.join("screen-free-spoken-artifacts");
        if !artifact_root.exists() {
            std::fs::create_dir(&artifact_root)
                .map_err(|error| format!("create selected speech artifact root: {error}"))?;
        }
        let artifact = WavArtifactSelection::per_play_root_with_limits(
            &artifact_root,
            config.boot_id.clone(),
            config.offer_generation,
            WavArtifactRetentionLimits::new(1024, 1024 * 1024 * 1024)?,
        )?;
        let adapter = discovery
            .clone()
            .initialize(
                config.host_id.clone(),
                config.boot_id.clone(),
                config.offer_generation,
                "grant/installed-screen-free-selected-speech".into(),
                Duration::from_secs(30),
            )
            .map_err(|error| format!("initialize selected speech provider: {error:?}"))?;
        let mut host = StdHost::new_with_playback(
            config.clone(),
            StdHostComposition::minimal().with_text(),
            selection.clone().with_bounded_speech_queue(),
        )?;
        host.set_initial_kernel_play_sequence(reserve_play_sequence_range(
            state_dir,
            config.boot_id.as_str(),
        )?)?;
        host.attach_espeak_speech_and_wav_artifact(adapter, artifact)?;
        Ok(Self {
            discovery,
            config,
            selection,
            authorization,
            voice: voice.into(),
            host: Arc::new(Mutex::new(host)),
            remaining_plays: Arc::new(AtomicU16::new(PLAYS_PER_ENCOUNTER as u16)),
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
        self.remaining_plays
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |remaining| {
                remaining.checked_sub(1)
            })
            .map_err(|_| {
                SpokenStreamExecutionRefusal::Plan(
                    "screen-free selected Play sequence range is exhausted".into(),
                )
            })?;
        let mut host = self.host.lock().map_err(|_| {
            SpokenStreamExecutionRefusal::Plan("selected speech Host lock failed".into())
        })?;
        if host.advertisement().host_id != self.config.host_id
            || host.advertisement().boot_id != self.config.boot_id
            || host.advertisement().offer_generation != self.config.offer_generation
        {
            return Err(SpokenStreamExecutionRefusal::Plan(
                "selected speech Host changed before Play".into(),
            ));
        }
        execute_spoken_batch_on_attached_host_with_capture(
            face,
            show,
            batch,
            &conduit_language::LanguageRequest::new(
                conduit_language::LanguageId::new("language/english".into())
                    .expect("English mechanical Mask Language"),
                None,
                conduit_language::LanguageVarietyPolicy::LanguageSufficient,
            )
            .expect("explicit mechanical Mask request"),
            &self.selection.clone().with_bounded_speech_queue(),
            &self.authorization,
            control,
            &mut host,
        )
    }

    pub(super) fn verify_receipt(
        &self,
        face: &Presentation,
        show: &MaskShow,
        batch: &SpokenBatch,
        result: &SpokenPlaybackExecution,
    ) -> Result<Vec<serde_json::Value>, String> {
        let spoken_segments = verified_spoken_segments(face, show, batch)?;
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
        if result.outcome == SpokenPlaybackOutcome::Completed {
            let capture = result
                .same_play_capture
                .as_ref()
                .ok_or("completed selected speaker Play omitted same-Play WAV")?;
            if u32::from(capture.pcm_blocks) != result.playback.metrics.blocks_committed
                || u64::from(capture.pcm_bytes) / 4 != result.playback.metrics.frames_committed
            {
                return Err("selected speaker WAV differs from committed PCM".into());
            }
        } else if result.same_play_capture.is_some() {
            return Err("incomplete selected speaker Play claimed a completed WAV".into());
        }
        Ok(spoken_segments)
    }

    pub(super) fn receipt_json(
        &self,
        result: &SpokenPlaybackExecution,
        spoken_segments: &[serde_json::Value],
    ) -> serde_json::Value {
        serde_json::json!({
            "schema": "conduit.body/spoken-face-playback@1",
            "outcome": format!("{:?}", result.outcome),
            "face_id": result.face_id,
            "face_revision": result.face_revision,
            "face_revision_decimal": result.face_revision.to_string(),
            "source_show_id": result.source_show_id,
            "stream_identity": result.stream_identity,
            "source_segments_sha256": result.source_segments_sha256,
            "spoken_segments": spoken_segments,
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
            "same_play_capture": result.same_play_capture.as_ref().map(|capture| serde_json::json!({
                "wav_artifact_locator": capture.wav_path,
                "wav_sha256": capture.wav_sha256,
                "wav_bytes": capture.wav_bytes,
                "pcm_sha256": capture.pcm_sha256,
                "pcm_bytes": capture.pcm_bytes,
                "pcm_blocks": capture.pcm_blocks,
            })),
        })
    }
}

/// Preserve exact committed words and all fields in the canonical source
/// digest. The caller may publish these only after the same Play's receipt has
/// been checked against this validated, one-segment Birth batch.
pub(super) fn verified_spoken_segments(
    face: &Presentation,
    show: &MaskShow,
    batch: &SpokenBatch,
) -> Result<Vec<serde_json::Value>, String> {
    batch
        .validate(face, show)
        .map_err(|error| format!("selected speech source digest refused: {error:?}"))?;
    if batch.segments.len() != 1 || batch.segments[0].segment.text.len() > 64 {
        return Err("selected Birth speech exceeds its one-segment, 64-byte batch bound".into());
    }
    Ok(batch
        .segments
        .iter()
        .map(|item| {
            serde_json::json!({
                "sequence": item.segment.sequence,
                "text": item.segment.text,
                "text_sha256": item.text_sha256,
                "reason_code": 1,
                "face_id": item.face_id,
                "face_revision_decimal": item.face_revision.to_string(),
                "show_id": item.show_id,
                "clause_index": item.clause_index,
                "clause_provenance_debug": format!("{:?}", item.clause_provenance),
            })
        })
        .collect())
}

#[cfg(test)]
mod play_sequence_tests {
    use super::*;

    #[test]
    fn repeated_encounters_on_one_boot_reserve_distinct_play_identities() {
        let root = std::env::temp_dir().join(format!(
            "conduit-screen-free-play-sequence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let first = reserve_play_sequence_range(&root, "boot/current").unwrap();
        let second = reserve_play_sequence_range(&root, "boot/current").unwrap();
        assert_eq!(first, SCREEN_FREE_SEQUENCE_START);
        assert_eq!(second, first + PLAYS_PER_ENCOUNTER);
        let plan = conduit_core::PlanId::from("plan/repeated-reading");
        let host = conduit_core::HostId::from("host/owner");
        let boot = conduit_core::BootId::from("boot/current");
        let original = conduit_core::bind_active_play(&plan, &host, &boot, first);
        let replay = conduit_core::bind_active_play(&plan, &host, &boot, second);
        assert_ne!(original.active_play_id, replay.active_play_id);
        assert_eq!(original.boot_id, replay.boot_id);
        std::fs::write(
            root.join("screen-free-play-sequence.pending"),
            "interrupted",
        )
        .unwrap();
        assert!(reserve_play_sequence_range(&root, "boot/current")
            .unwrap_err()
            .contains("interrupted update"));
        std::fs::remove_file(root.join("screen-free-play-sequence.pending")).unwrap();
        assert_eq!(
            reserve_play_sequence_range(&root, "boot/new").unwrap(),
            first
        );
        std::fs::write(root.join("screen-free-play-sequence"), "").unwrap();
        assert!(reserve_play_sequence_range(&root, "boot/new")
            .unwrap_err()
            .contains("has no Boot"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
