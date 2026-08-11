use crate::pcm::deterministic_pcm;
use crate::{OutputCondition, SpeechOutcome, SPECIMEN_TEXT};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeechSign {
    Synthesized { pcm_bytes: u32, pcm_sha256: String },
    Presented { condition: OutputCondition },
    Degraded { wav_bytes: u32, wav_sha256: String },
    Refused { reason: String },
    Failed { reason: String },
    Cancelled,
    Terminal,
}

pub(crate) fn outcome_signs(outcome: &SpeechOutcome) -> Vec<SpeechSign> {
    let mut signs = match outcome {
        SpeechOutcome::Played { pcm_sha256 } => vec![
            SpeechSign::Synthesized {
                pcm_bytes: u32::try_from(deterministic_pcm(SPECIMEN_TEXT).len()).unwrap(),
                pcm_sha256: pcm_sha256.clone(),
            },
            SpeechSign::Presented {
                condition: OutputCondition::PrimaryPlayback,
            },
        ],
        SpeechOutcome::WavArtifact {
            wav_bytes,
            wav_sha256,
            pcm_sha256,
        } => vec![
            SpeechSign::Synthesized {
                pcm_bytes: u32::try_from(deterministic_pcm(SPECIMEN_TEXT).len()).unwrap(),
                pcm_sha256: pcm_sha256.clone(),
            },
            SpeechSign::Degraded {
                wav_bytes: *wav_bytes,
                wav_sha256: wav_sha256.clone(),
            },
        ],
        SpeechOutcome::FormatMismatch => refused("format-mismatch"),
        SpeechOutcome::Pressure => refused("buffer-pressure"),
        SpeechOutcome::ImplementationUnavailable => refused("implementation-unavailable"),
        SpeechOutcome::BaseDenied => refused("base-denied"),
        SpeechOutcome::Cancelled => vec![SpeechSign::Cancelled],
        SpeechOutcome::Underrun => failed("underrun"),
        SpeechOutcome::BaseLost => failed("base-lost"),
        SpeechOutcome::DeviceFailure => failed("device-output-failure"),
    };
    signs.push(SpeechSign::Terminal);
    signs
}

fn refused(reason: &str) -> Vec<SpeechSign> {
    vec![SpeechSign::Refused {
        reason: reason.into(),
    }]
}

fn failed(reason: &str) -> Vec<SpeechSign> {
    vec![SpeechSign::Failed {
        reason: reason.into(),
    }]
}
