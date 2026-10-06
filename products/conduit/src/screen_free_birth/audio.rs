//! An explicitly selected hosted speech effect for the zero-Body Birth Face.
//!
//! This advances one bounded closing Flow at a time. It never treats a text
//! readout, source Show, or produced WAV as evidence of speaker playback.

use std::path::{Path, PathBuf};

use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::hosted_speech_synthesis::EspeakDiscovery;
use conduit_std_host::spoken_face_mask::{
    SpokenBatch, SpokenBatchDelivery, SpokenFaceSession, SpokenTurnReceipt,
};
use conduit_std_host::spoken_face_stream_execution::{
    execute_real_spoken_batch, SpokenStreamExecution,
};

pub(super) struct BirthSpeechOutput {
    provider: EspeakDiscovery,
    directory: PathBuf,
    next_artifact: u64,
}

pub(super) struct SpokenBirthStep {
    pub batch: SpokenBatch,
    pub execution: SpokenStreamExecution,
    pub terminal: Option<SpokenTurnReceipt>,
}

impl BirthSpeechOutput {
    // The installed spoken entrance will supply the selected Host provider;
    // the present public Birth entrance has no such selection yet.
    #[allow(dead_code)]
    pub fn new(provider: EspeakDiscovery, directory: &Path) -> Result<Self, String> {
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("speech output needs an existing absolute directory".into());
        }
        Ok(Self {
            provider,
            directory: directory.to_path_buf(),
            next_artifact: 0,
        })
    }

    /// A caller may request Stop between completed batches. The current
    /// synchronous Host Play has no channel for interrupting a running batch.
    pub fn advance(
        &mut self,
        reader: &mut SpokenFaceSession,
        face: &Presentation,
        show: &MaskShow,
    ) -> Result<Option<SpokenBirthStep>, String> {
        // The Crèche's full semantic readout is longer than one admitted
        // 30-second speech Play. Its technical option lists can also exceed
        // that budget as one 451-byte segment, so request smaller UTF-8
        // pieces. The effect still measures actual PCM and may refuse.
        let Some(batch) = reader.next_batch_with_limits(1, 64).map_err(debug_error)? else {
            return Ok(None);
        };
        self.next_artifact = self
            .next_artifact
            .checked_add(1)
            .ok_or("speech artifact count exhausted")?;
        let wav = self
            .directory
            .join(format!("birth-speech-{:06}.wav", self.next_artifact));
        let execution = match execute_real_spoken_batch(
            face,
            show,
            &batch,
            self.provider.clone(),
            &conduit_language::LanguageRequest::new(
                conduit_language::LanguageId::new("language/english".into())
                    .expect("English mechanical Mask Language"),
                None,
                conduit_language::LanguageVarietyPolicy::LanguageSufficient,
            )
            .expect("explicit mechanical Mask request"),
            &wav,
        ) {
            Ok(execution) => execution,
            Err(refusal) => {
                reader
                    .acknowledge_batch(SpokenBatchDelivery::Failed(format!("{refusal:?}")))
                    .map_err(debug_error)?;
                let source = &batch.segments[0];
                return Err(format!(
                        "speech Host refused Face clause {:?}, segment sha256={}, text bytes={}: {refusal:?}",
                        source.clause_index,
                        source.text_sha256,
                        source.segment.text.len()
                    ));
            }
        };
        let terminal = reader
            .acknowledge_batch(SpokenBatchDelivery::Completed(execution.receipt.clone()))
            .map_err(debug_error)?;
        Ok(Some(SpokenBirthStep {
            batch,
            execution,
            terminal,
        }))
    }
}

fn debug_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
