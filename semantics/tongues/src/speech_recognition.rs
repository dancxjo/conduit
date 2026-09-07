//! Portable bounded speech-recognition meaning and one exact fixture adapter.

use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, AUDIO_PCM_INFO_ID};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{string::String, vec, vec::Vec};

pub const SPEECH_RECOGNIZE_KIND: &str = "speech/recognize";
pub const SPEECH_RECOGNIZE_REVISION: &str = "conduit.speech/recognize@1";
pub const SPEECH_RECOGNITION_RESULT_KIND: &str = "speech/recognition-result@1";
pub const SPEECH_RECOGNITION_TO_TEXT_KIND: &str = "speech/recognition-to-text";
pub const SPEECH_RECOGNITION_TO_TEXT_REVISION: &str = "conduit.speech/recognition-to-text@1";
pub const MAXIMUM_RECOGNIZED_TEXT_BYTES: usize = 256;
pub const MAXIMUM_RECOGNITION_RESULT_BYTES: usize = 2_048;
pub const MAXIMUM_RECOGNITION_FIXTURES: usize = 8;
pub const MAXIMUM_RECOGNITION_AUDIO_BYTES: usize = 32_768;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechRecognitionContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SpeechRecognitionDisposition {
    Recognized,
    NoSpeech,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpeechRecognitionResult {
    pub disposition: SpeechRecognitionDisposition,
    pub text: Option<String>,
    pub audio_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpeechRecognitionAttempt {
    Result(SpeechRecognitionResult),
    ResourceUnavailable,
    Failed { audio_sha256: [u8; 32] },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpeechRecognitionRefusal {
    EmptyFixtures,
    TooManyFixtures,
    EmptyTranscript,
    TranscriptTooLarge,
    DuplicateAudio,
    AudioTooLarge,
    InvalidPcm,
    UnsupportedPcmProfile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpeechRecognitionValueError {
    BoundExceeded,
    Malformed,
    NonCanonical,
    InvalidValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecognitionTextRefusal {
    InvalidResult,
    NotRecognized,
}

#[derive(Serialize, Deserialize)]
struct SpeechRecognitionValue {
    schema: String,
    result: SpeechRecognitionResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedFixture {
    audio_sha256: [u8; 32],
    transcript: String,
}

/// Exact recorded-audio oracle for deterministic conformance, not a production
/// recognizer or a claim about a microphone, model, or ambient resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedSpeechRecognizer {
    fixtures: Vec<RecordedFixture>,
}

pub fn speech_recognition_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(SPEECH_RECOGNIZE_KIND),
        kind_contract_revision: KindContractRevision::from(SPEECH_RECOGNIZE_REVISION),
        inputs: vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Input)],
        outputs: vec![port(
            "result",
            SPEECH_RECOGNITION_RESULT_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
        },
    }
}

pub fn speech_recognition_to_text_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(SPEECH_RECOGNITION_TO_TEXT_KIND),
        kind_contract_revision: KindContractRevision::from(SPEECH_RECOGNITION_TO_TEXT_REVISION),
        inputs: vec![port(
            "result",
            SPEECH_RECOGNITION_RESULT_KIND,
            PortDirection::Input,
        )],
        outputs: vec![port(
            "text",
            conduit_text::TEXT_VALUE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
        },
    }
}

pub fn install_speech_recognition_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for contract in [
        speech_recognition_contract(),
        speech_recognition_to_text_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn encode_speech_recognition_result(
    result: &SpeechRecognitionResult,
) -> Result<Vec<u8>, SpeechRecognitionValueError> {
    validate_result(result)?;
    let encoded = serde_json::to_vec(&SpeechRecognitionValue {
        schema: "conduit.speech/recognition-result-value@1".into(),
        result: result.clone(),
    })
    .map_err(|_| SpeechRecognitionValueError::Malformed)?;
    if encoded.len() > MAXIMUM_RECOGNITION_RESULT_BYTES {
        return Err(SpeechRecognitionValueError::BoundExceeded);
    }
    Ok(encoded)
}

pub fn decode_speech_recognition_result(
    encoded: &[u8],
) -> Result<SpeechRecognitionResult, SpeechRecognitionValueError> {
    if encoded.len() > MAXIMUM_RECOGNITION_RESULT_BYTES {
        return Err(SpeechRecognitionValueError::BoundExceeded);
    }
    let value: SpeechRecognitionValue =
        serde_json::from_slice(encoded).map_err(|_| SpeechRecognitionValueError::Malformed)?;
    if value.schema != "conduit.speech/recognition-result-value@1" {
        return Err(SpeechRecognitionValueError::InvalidValue);
    }
    validate_result(&value.result)?;
    if encode_speech_recognition_result(&value.result)? != encoded {
        return Err(SpeechRecognitionValueError::NonCanonical);
    }
    Ok(value.result)
}

pub fn project_recognized_text(encoded: &[u8]) -> Result<Vec<u8>, RecognitionTextRefusal> {
    let result = decode_speech_recognition_result(encoded)
        .map_err(|_| RecognitionTextRefusal::InvalidResult)?;
    match (result.disposition, result.text) {
        (SpeechRecognitionDisposition::Recognized, Some(text)) => Ok(text.into_bytes()),
        (SpeechRecognitionDisposition::NoSpeech, None) => {
            Err(RecognitionTextRefusal::NotRecognized)
        }
        _ => Err(RecognitionTextRefusal::InvalidResult),
    }
}

fn validate_result(result: &SpeechRecognitionResult) -> Result<(), SpeechRecognitionValueError> {
    match (&result.disposition, &result.text) {
        (SpeechRecognitionDisposition::Recognized, Some(text))
            if !text.is_empty() && text.len() <= MAXIMUM_RECOGNIZED_TEXT_BYTES =>
        {
            Ok(())
        }
        (SpeechRecognitionDisposition::NoSpeech, None) => Ok(()),
        _ => Err(SpeechRecognitionValueError::InvalidValue),
    }
}

impl RecordedSpeechRecognizer {
    pub fn new(fixtures: &[(&[u8], &str)]) -> Result<Self, SpeechRecognitionRefusal> {
        if fixtures.is_empty() {
            return Err(SpeechRecognitionRefusal::EmptyFixtures);
        }
        if fixtures.len() > MAXIMUM_RECOGNITION_FIXTURES {
            return Err(SpeechRecognitionRefusal::TooManyFixtures);
        }
        let mut admitted = Vec::with_capacity(fixtures.len());
        for (audio, transcript) in fixtures {
            validate_audio(audio)?;
            if transcript.is_empty() {
                return Err(SpeechRecognitionRefusal::EmptyTranscript);
            }
            if transcript.len() > MAXIMUM_RECOGNIZED_TEXT_BYTES {
                return Err(SpeechRecognitionRefusal::TranscriptTooLarge);
            }
            let audio_sha256 = digest(audio);
            if admitted
                .iter()
                .any(|fixture: &RecordedFixture| fixture.audio_sha256 == audio_sha256)
            {
                return Err(SpeechRecognitionRefusal::DuplicateAudio);
            }
            admitted.push(RecordedFixture {
                audio_sha256,
                transcript: String::from(*transcript),
            });
        }
        Ok(Self { fixtures: admitted })
    }

    pub fn recognize(
        &self,
        audio: &[u8],
    ) -> Result<SpeechRecognitionAttempt, SpeechRecognitionRefusal> {
        let (_, payload) = validate_audio(audio)?;
        let audio_sha256 = digest(audio);
        if payload.iter().all(|sample| *sample == 0) {
            return Ok(SpeechRecognitionAttempt::Result(SpeechRecognitionResult {
                disposition: SpeechRecognitionDisposition::NoSpeech,
                text: None,
                audio_sha256,
            }));
        }
        if let Some(fixture) = self
            .fixtures
            .iter()
            .find(|fixture| fixture.audio_sha256 == audio_sha256)
        {
            return Ok(SpeechRecognitionAttempt::Result(SpeechRecognitionResult {
                disposition: SpeechRecognitionDisposition::Recognized,
                text: Some(fixture.transcript.clone()),
                audio_sha256,
            }));
        }
        Ok(SpeechRecognitionAttempt::Failed { audio_sha256 })
    }

    pub fn resource_unavailable() -> SpeechRecognitionAttempt {
        SpeechRecognitionAttempt::ResourceUnavailable
    }
}

fn validate_audio(audio: &[u8]) -> Result<(PcmFrameHeader, &[u8]), SpeechRecognitionRefusal> {
    if audio.len() > MAXIMUM_RECOGNITION_AUDIO_BYTES {
        return Err(SpeechRecognitionRefusal::AudioTooLarge);
    }
    let (header, payload) =
        PcmFrameHeader::decode_frame(audio).map_err(|_| SpeechRecognitionRefusal::InvalidPcm)?;
    if header.representation != PcmSampleRepresentation::Signed16LittleEndian
        || header.layout != PcmChannelLayout::Mono
        || header.sample_rate_hz != 16_000
        || header.discontinuity
    {
        return Err(SpeechRecognitionRefusal::UnsupportedPcmProfile);
    }
    Ok((header, payload))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}
