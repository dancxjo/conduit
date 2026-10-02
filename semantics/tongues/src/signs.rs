use crate::{OutputCondition, SpeechOutcome, SpeechSign};
pub(crate) fn outcome_signs(outcome: &SpeechOutcome, pcm_bytes: Option<u32>) -> Vec<SpeechSign> {
    let mut signs = match outcome {
        SpeechOutcome::Played(payload) => vec![
            SpeechSign::synthesized(
                pcm_bytes.expect("played speech has synthesized PCM"),
                payload.pcm_sha256().clone(),
            )
            .expect("valid synthesized sign"),
            SpeechSign::presented(OutputCondition::PrimaryPlayback).expect("valid presented sign"),
        ],
        SpeechOutcome::WavArtifact(payload) => vec![
            SpeechSign::synthesized(
                pcm_bytes.expect("WAV speech has synthesized PCM"),
                payload.pcm_sha256().clone(),
            )
            .expect("valid synthesized sign"),
            SpeechSign::degraded(*payload.wav_bytes(), payload.wav_sha256().clone())
                .expect("valid degraded sign"),
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
    vec![SpeechSign::refused(
        crate::SpeechSignReason::new(reason.into()).expect("static refusal reason is valid"),
    )
    .expect("valid refusal sign")]
}

fn failed(reason: &str) -> Vec<SpeechSign> {
    vec![SpeechSign::failed(
        crate::SpeechSignReason::new(reason.into()).expect("static failure reason is valid"),
    )
    .expect("valid failure sign")]
}
