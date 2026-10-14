//! Actual ordered artifact Plays of one privately selected checked spoken Look.
//! Successful artifact acknowledgements advance this session; public receipts cannot.
use crate::{
    direct_spoken_mask_runtime::DirectSpokenMaskPreparation,
    spoken_face_stream_execution::{
        execute_spoken_batch_on_attached_artifact_host, SpokenStreamExecution,
    },
    spoken_template_reading::PreparedSpokenTemplateReading,
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, RunControl, StdHost,
    TimerAdapter,
};
use conduit_core::{port_id, ConnectionTrack, ObservationKind, PlanFragment, TerminalDisposition};
use conduit_face::{DirectArtifactAcknowledgedSpokenShow, Face, MaskShow, SpokenTemplateWording};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize)]
pub struct SpokenTemplateFrameReceipt {
    pub frame_index: u16,
    pub text_bytes: u32,
    pub text_sha256: [u8; 32],
    pub host_id: String,
    pub boot_id: String,
    pub source_face_id: String,
    pub source_face_revision: u64,
    pub source_show_id: String,
    pub speech_plan_id: String,
    pub speech_play_id: String,
    pub artifact_locator: std::path::PathBuf,
    pub artifact_sha256: String,
    /// Opening acknowledges PCM bytes; continuation receipts hash complete WAV bytes.
    pub artifact_digest_scope: &'static str,
    pub pcm_bytes: u32,
    pub pcm_blocks: u16,
    pub completion_sign_id: Option<String>,
    pub provider_sha256: Option<String>,
    pub source_segments_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpokenTemplateExecutionSummary {
    pub template: SpokenTemplateWording,
    pub source_show: MaskShow,
    pub frames: Vec<SpokenTemplateFrameReceipt>,
    pub completed_wording_bytes: usize,
    pub completed_wording_sha256: [u8; 32],
    pub proof_class: &'static str,
}

pub struct SpokenTemplateExecutionSession {
    reading: PreparedSpokenTemplateReading,
    face: Face,
    opening: DirectArtifactAcknowledgedSpokenShow,
    next: usize,
    receipts: Vec<SpokenTemplateFrameReceipt>,
    retired: bool,
}

struct OneShow(Option<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for OneShow {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.front_port_id.as_str() != "show"
            || value.track != ConnectionTrack::Payload
            || value.value_kind.as_str() != conduit_face::SHOW_VALUE_KIND
            || value.sequence != 0
            || self.0.is_some()
        {
            return Err("checked template opening must emit exactly one Show".into());
        }
        self.0 = Some(value);
        Ok(())
    }
}
struct NoWait;
impl TimerAdapter for NoWait {
    fn wait(&mut self, _: std::time::Duration) {}
}

impl StdHost {
    /// A session can only be minted by this actual completed opening kernel Play.
    /// The opaque reading and private preparation must belong to this current Host.
    pub fn execute_prepared_spoken_template_opening(
        &mut self,
        face: &Face,
        fragment: PlanFragment,
        preparation: DirectSpokenMaskPreparation,
        reading: PreparedSpokenTemplateReading,
        control: &RunControl,
    ) -> Result<SpokenTemplateExecutionSession, String> {
        if control.stop_requested() {
            return Err("checked template opening cancelled before Play".into());
        }
        reading.validate_current(self, face)?;
        preparation.validate_template_host(self.advertisement())?;
        let expected_namespace = match reading.receipt().template_identity.as_str() {
            "spoken/brief@1" => "owner_spoken_brief_template_mask",
            "spoken/expanded@1" => "owner_spoken_expanded_template_mask",
            _ => return Err("unknown privately selected template".into()),
        };
        if preparation.face != *face
            || preparation.opening_wording()? != reading.opening_wording()
            || preparation.planned_mask.mask.plot_name != expected_namespace
            || preparation.planned_mask.plan.fragments.as_slice()
                != core::slice::from_ref(&fragment)
        {
            return Err("checked template opening tuple differs from selected source/Mask".into());
        }
        let selected_evidence = preparation
            .checked_template()
            .ok_or("opening preparation has no private template evidence")?;
        if selected_evidence.wording != *reading.receipt()
            || selected_evidence.opening_wording != reading.opening_wording()
            || usize::from(selected_evidence.frame_count) != reading.frames_count()
        {
            return Err("opening preparation differs from opaque checked template reading".into());
        }
        let expected_mask = preparation.planned_mask.clone();
        let input = ExternalForeInput {
            front_port_id: port_id("face"),
            track: ConnectionTrack::Payload,
            bytes: preparation.encoded_face().to_vec(),
        };
        let mut receipts = Vec::with_capacity(reading.frames_count());
        let mut capture = OneShow(None);
        let report = self.run_direct_spoken_mask_controlled_to(
            fragment,
            preparation,
            &[input],
            &mut capture,
            &mut std::io::sink(),
            &mut NoWait,
            control,
        )?;
        if control.stop_requested()
            || !matches!(
                report.observations.last().map(|value| &value.kind),
                Some(ObservationKind::PlanTerminal {
                    disposition: TerminalDisposition::Completed
                })
            )
        {
            return Err("checked template opening did not complete".into());
        }
        let kernel = report
            .kernel
            .ok_or("checked template opening has no actual kernel Play")?;
        let delivery = capture
            .0
            .ok_or("checked template opening has no actual Show")?;
        let opening: DirectArtifactAcknowledgedSpokenShow = serde_json::from_slice(&delivery.bytes)
            .map_err(|error| format!("decode actual template Show: {error}"))?;
        opening
            .validate(face)
            .map_err(|error| format!("actual template Show: {error:?}"))?;
        let evidence = opening
            .checked_template
            .as_ref()
            .ok_or("actual template Show omitted source evidence")?;
        if opening.show.planned_mask != expected_mask
            || opening.show.show.active_play_id != kernel.active_play_id
            || evidence.wording != *reading.receipt()
            || evidence.opening_wording != reading.opening_wording()
            || usize::from(evidence.frame_count) != reading.frames_count()
            || opening.show.show.host_id != self.advertisement().host_id
            || opening.show.show.boot_id != self.advertisement().boot_id
        {
            return Err(
                "actual opening acknowledgement differs from privately selected template/Play"
                    .into(),
            );
        }
        reading.validate_current(self, face)?;
        let locator = std::path::PathBuf::from(
            opening
                .artifact
                .artifact_locator
                .as_ref()
                .ok_or("opening WAV locator absent")?,
        );
        let metadata = std::fs::metadata(&locator)
            .map_err(|error| format!("actual opening metadata: {error}"))?;
        if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
            return Err("opening artifact exceeds bounded retained WAV extent".into());
        }
        let wav = std::fs::read(&locator)
            .map_err(|error| format!("read actual opening artifact: {error}"))?;
        if wav.len() != opening.artifact.pcm_bytes as usize + 44
            || wav.get(..4) != Some(b"RIFF")
            || wav.get(8..12) != Some(b"WAVE")
            || format!("{:x}", Sha256::digest(&wav[44..])) != opening.artifact.content_sha256
        {
            return Err("actual opening artifact bytes differ from completed receipt".into());
        }
        receipts.push(SpokenTemplateFrameReceipt {
            frame_index: 0,
            text_bytes: reading.opening_wording().len() as u32,
            text_sha256: Sha256::digest(reading.opening_wording().as_bytes()).into(),
            host_id: opening.show.show.host_id.as_str().into(),
            boot_id: opening.show.show.boot_id.as_str().into(),
            source_face_id: face.identity.as_str().into(),
            source_face_revision: face.revision,
            source_show_id: opening.show.show_id.as_str().into(),
            speech_plan_id: opening.artifact.plan_id.as_str().into(),
            speech_play_id: opening.artifact.active_play_id.as_str().into(),
            artifact_locator: locator,
            artifact_sha256: opening.artifact.content_sha256.clone(),
            artifact_digest_scope: "pcm",
            pcm_bytes: opening.artifact.pcm_bytes,
            pcm_blocks: opening.artifact.blocks,
            completion_sign_id: Some(opening.artifact.completion_sign_id.as_str().into()),
            provider_sha256: Some(
                self.speech_synthesis
                    .as_ref()
                    .ok_or("opening selected provider absent")?
                    .provider_sha256()
                    .into(),
            ),
            source_segments_sha256: None,
        });
        Ok(SpokenTemplateExecutionSession {
            reading,
            face: face.clone(),
            opening,
            next: 1,
            receipts,
            retired: false,
        })
    }
}

impl SpokenTemplateExecutionSession {
    pub fn show(&self) -> &MaskShow {
        &self.opening.show
    }
    pub fn opening(&self) -> &DirectArtifactAcknowledgedSpokenShow {
        &self.opening
    }
    pub fn template(&self) -> &SpokenTemplateWording {
        self.reading.receipt()
    }
    pub fn next_index(&self) -> usize {
        self.next
    }
    pub fn completed_count(&self) -> usize {
        self.receipts.len()
    }
    pub fn receipts(&self) -> &[SpokenTemplateFrameReceipt] {
        &self.receipts
    }
    pub fn complete(&self) -> bool {
        !self.retired && self.next == self.reading.frames_count()
    }

    pub fn read_next(
        &mut self,
        host: &mut StdHost,
        current_face: &Face,
        current_show: &MaskShow,
        control: &RunControl,
    ) -> Result<SpokenStreamExecution, String> {
        let result = self.read_next_inner(host, current_face, current_show, control);
        if result.is_err() {
            self.retired = true;
        }
        result
    }
    fn read_next_inner(
        &mut self,
        host: &mut StdHost,
        face: &Face,
        show: &MaskShow,
        control: &RunControl,
    ) -> Result<SpokenStreamExecution, String> {
        if self.retired || control.stop_requested() {
            return Err("spoken template session retired or cancelled".into());
        }
        self.reading.validate_current(host, face)?;
        if face != &self.face || show != self.show() {
            return Err("spoken template current Face/Show changed".into());
        }
        let index = self.next;
        let batch = self.reading.prepare_frame_batch(host, face, show, index)?;
        // Matches the direct Mask's checked English request; no locale inference.
        let language = conduit_language::LanguageRequest::new(
            conduit_language::LanguageId::new("language/english".into())
                .map_err(|e| format!("English: {e:?}"))?,
            None,
            conduit_language::LanguageVarietyPolicy::LanguageSufficient,
        )
        .map_err(|e| format!("English: {e:?}"))?;
        let execution = execute_spoken_batch_on_attached_artifact_host(
            face, show, &batch, &language, control, host,
        )
        .map_err(|error| format!("actual template frame Play: {error:?}"))?;
        if control.stop_requested() {
            return Err("spoken template frame cancelled at acknowledgement".into());
        }
        self.reading.validate_current(host, face)?;
        if execution.host_id != host.advertisement().host_id.as_str()
            || execution.boot_id != host.advertisement().boot_id.as_str()
            || execution.receipt.source_show_id != show.show_id.as_str()
            || execution.receipt.stream_identity != batch.stream_identity
            || execution.receipt.source_segments_sha256 != batch.source_segments_sha256
            || execution.receipt.speech_plan_id.is_empty()
            || execution.receipt.speech_play_id.is_empty()
            || self.receipts[0].provider_sha256.as_deref()
                != Some(execution.receipt.provider_sha256.as_str())
            || execution.receipt.pcm_bytes == 0
            || execution.receipt.pcm_blocks == 0
        {
            return Err("actual continuation receipt differs from current template batch".into());
        }
        if self
            .receipts
            .iter()
            .any(|receipt| receipt.speech_play_id == execution.receipt.speech_play_id)
        {
            return Err("continuation reused an already completed speech Play".into());
        }
        if self.receipts.len() == self.receipts.capacity() {
            return Err("spoken template admitted receipt storage exhausted".into());
        }
        let text = self.reading.framed_wording(index)?;
        self.receipts.push(SpokenTemplateFrameReceipt {
            frame_index: index as u16,
            text_bytes: text.len() as u32,
            text_sha256: Sha256::digest(text.as_bytes()).into(),
            host_id: execution.host_id.clone(),
            boot_id: execution.boot_id.clone(),
            source_face_id: face.identity.as_str().into(),
            source_face_revision: face.revision,
            source_show_id: show.show_id.as_str().into(),
            speech_plan_id: execution.receipt.speech_plan_id.clone(),
            speech_play_id: execution.receipt.speech_play_id.clone(),
            artifact_locator: execution.wav_path.clone(),
            artifact_sha256: execution.receipt.wav_sha256.clone(),
            artifact_digest_scope: "wav",
            pcm_bytes: execution.receipt.pcm_bytes,
            pcm_blocks: execution.receipt.pcm_blocks,
            completion_sign_id: None,
            provider_sha256: Some(execution.receipt.provider_sha256.clone()),
            source_segments_sha256: Some(execution.receipt.source_segments_sha256.clone()),
        });
        self.next += 1;
        Ok(execution)
    }

    /// Documentary aggregate only; it cannot mint or resume a session.
    pub fn finish(&self) -> Result<SpokenTemplateExecutionSummary, String> {
        if !self.complete() || self.receipts.len() != self.reading.frames_count() {
            return Err("spoken template reading is incomplete or retired".into());
        }
        let mut reconstructed = String::new();
        let mut plays = std::collections::BTreeSet::new();
        for (index, receipt) in self.receipts.iter().enumerate() {
            let text = self.reading.framed_wording(index)?;
            if usize::from(receipt.frame_index) != index
                || receipt.text_bytes as usize != text.len()
                || receipt.text_sha256 != <[u8; 32]>::from(Sha256::digest(text.as_bytes()))
                || receipt.source_face_id != self.face.identity.as_str()
                || receipt.source_face_revision != self.face.revision
                || receipt.source_show_id != self.show().show_id.as_str()
                || receipt.host_id != self.show().show.host_id.as_str()
                || receipt.boot_id != self.show().show.boot_id.as_str()
                || !plays.insert(&receipt.speech_play_id)
            {
                return Err(
                    "spoken template ordered receipts do not cover exact source frames".into(),
                );
            }
            reconstructed.push_str(text);
        }
        if reconstructed != self.template().wording
            || <[u8; 32]>::from(Sha256::digest(reconstructed.as_bytes()))
                != self.template().wording_digest
        {
            return Err("completed artifacts do not cover exact full template wording".into());
        }
        Ok(SpokenTemplateExecutionSummary {
            template: self.template().clone(),
            source_show: self.show().clone(),
            frames: self.receipts.clone(),
            completed_wording_bytes: reconstructed.len(),
            completed_wording_sha256: self.template().wording_digest,
            proof_class: "actual ordered same-Host WAV artifact Plays; no speaker or human hearing",
        })
    }
}

#[cfg(test)]
#[path = "spoken_template_execution/tests.rs"]
mod tests;
