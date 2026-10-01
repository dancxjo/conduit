//! Portable bounded speech-recognition meaning and one exact fixture adapter.

use conduit_audio::{
    PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, AUDIO_PCM_CLIP_INFO_ID,
    AUDIO_PCM_INFO_ID, MAXIMUM_PCM_CLIP_BYTES,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindId, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{
    rust_binding::NativeRustBinding, KindSignature, ProfileCatalog, StartupCatalog,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{string::String, vec, vec::Vec};

use crate::{
    committed_recognition_turn_contract, committed_turn_to_text_contract,
    streaming_speech_recognition_contract, RecognitionTextRefusal, SpeechRecognitionAttempt,
    SpeechRecognitionDisposition, SpeechRecognitionRefusal, SpeechRecognitionResult,
    SpeechRecognitionValueError,
};

pub const SPEECH_RECOGNIZE_KIND: &str = "speech/recognize";
pub const SPEECH_RECOGNIZE_REVISION: &str = "conduit.speech/recognize@1";
pub const SPEECH_RECOGNIZE_CLIP_KIND: &str = "speech/recognize-clip";
pub const SPEECH_RECOGNIZE_CLIP_REVISION: &str = "conduit.speech/recognize-clip@1";
pub const SPEECH_RECOGNITION_RESULT_KIND: &str = "speech/recognition-result@2";
pub const MAXIMUM_RECOGNITION_PROVIDER_IDENTITY_BYTES: usize = 128;
pub const SPEECH_RECOGNITION_TO_TEXT_KIND: &str = "speech/recognition-to-text";
pub const SPEECH_RECOGNITION_TO_TEXT_REVISION: &str = "conduit.speech/recognition-to-text@1";
pub const MAXIMUM_RECOGNIZED_TEXT_BYTES: usize = 256;
pub const MAXIMUM_RECOGNITION_RESULT_BYTES: usize = 4_096;
pub const RECOGNITION_RESULT_QUEUE_BYTES: u32 = MAXIMUM_RECOGNITION_RESULT_BYTES as u32;
pub const MAXIMUM_RECOGNITION_FIXTURES: usize = 8;
pub const MAXIMUM_RECOGNITION_AUDIO_BYTES: usize = 32_768;

// The semantic type owns the meaning; this adapter preserves the established
// JSON representation of recognition results.
impl Serialize for SpeechRecognitionDisposition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::Recognized => "Recognized",
            Self::NoSpeech => "NoSpeech",
        })
    }
}

impl<'de> Deserialize<'de> for SpeechRecognitionDisposition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "Recognized" => Ok(Self::Recognized),
            "NoSpeech" => Ok(Self::NoSpeech),
            _ => Err(serde::de::Error::custom(
                "unknown speech recognition disposition",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechRecognitionContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

impl SpeechRecognitionContract {
    pub fn into_semantic_capability_contract(self) -> Kind {
        let shorthand = match (self.inputs.as_slice(), self.outputs.as_slice()) {
            ([input], [output]) => Some((input.port_id.clone(), output.port_id.clone())),
            _ => None,
        };
        Kind {
            startup_parameters: Vec::new(),
            shorthand,
            kind_id: self.kind_id,
            kind_contract_revision: self.kind_contract_revision,
            inputs: self.inputs,
            outputs: self.outputs,
            configuration: Default::default(),
            semantic_laws: Default::default(),
            limits: self.limits,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SpeechRecognitionValue {
    schema: String,
    result: SpeechRecognitionWireResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SpeechRecognitionWireResult {
    disposition: SpeechRecognitionDisposition,
    text: Option<String>,
    audio_sha256: [u8; 32],
    audio_extent_bytes: u32,
    provider_identity: String,
}

impl From<SpeechRecognitionResult> for SpeechRecognitionWireResult {
    fn from(result: SpeechRecognitionResult) -> Self {
        match result {
            SpeechRecognitionResult::Recognized(value) => Self {
                disposition: SpeechRecognitionDisposition::Recognized,
                text: Some(value.text().get().clone()),
                audio_sha256: *value.audio_sha256().get(),
                audio_extent_bytes: *value.audio_extent_bytes(),
                provider_identity: value.provider_identity().get().clone(),
            },
            SpeechRecognitionResult::NoSpeech(value) => Self {
                disposition: SpeechRecognitionDisposition::NoSpeech,
                text: None,
                audio_sha256: *value.audio_sha256().get(),
                audio_extent_bytes: *value.audio_extent_bytes(),
                provider_identity: value.provider_identity().get().clone(),
            },
        }
    }
}

impl TryFrom<SpeechRecognitionWireResult> for SpeechRecognitionResult {
    type Error = SpeechRecognitionValueError;

    fn try_from(value: SpeechRecognitionWireResult) -> Result<Self, Self::Error> {
        let digest = crate::SpeechRecognitionAudioDigest::new(value.audio_sha256)
            .map_err(|_| SpeechRecognitionValueError::InvalidValue)?;
        let provider = crate::SpeechRecognitionProviderIdentity::new(value.provider_identity)
            .map_err(|_| SpeechRecognitionValueError::InvalidValue)?;
        match (value.disposition, value.text) {
            (SpeechRecognitionDisposition::Recognized, Some(text)) => {
                let text = crate::RecognizedSpeechText::new(text)
                    .map_err(|_| SpeechRecognitionValueError::InvalidValue)?;
                SpeechRecognitionResult::recognized(
                    value.audio_extent_bytes,
                    digest,
                    provider,
                    text,
                )
                .map_err(|_| SpeechRecognitionValueError::InvalidValue)
            }
            (SpeechRecognitionDisposition::NoSpeech, None) => {
                SpeechRecognitionResult::no_speech(value.audio_extent_bytes, digest, provider)
                    .map_err(|_| SpeechRecognitionValueError::InvalidValue)
            }
            _ => Err(SpeechRecognitionValueError::InvalidValue),
        }
    }
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
        kind_contract_revision: KindIdentity::from(SPEECH_RECOGNIZE_REVISION),
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

pub fn speech_clip_recognition_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(SPEECH_RECOGNIZE_CLIP_KIND),
        kind_contract_revision: KindIdentity::from(SPEECH_RECOGNIZE_CLIP_REVISION),
        inputs: vec![port("clip", AUDIO_PCM_CLIP_INFO_ID, PortDirection::Input)],
        outputs: vec![port(
            "result",
            SPEECH_RECOGNITION_RESULT_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_PCM_CLIP_BYTES as u32,
        },
    }
}

pub fn speech_recognition_to_text_contract() -> SpeechRecognitionContract {
    SpeechRecognitionContract {
        kind_id: kind_id(SPEECH_RECOGNITION_TO_TEXT_KIND),
        kind_contract_revision: KindIdentity::from(SPEECH_RECOGNITION_TO_TEXT_REVISION),
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
            max_queue_bytes: RECOGNITION_RESULT_QUEUE_BYTES,
        },
    }
}

pub fn install_speech_recognition_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias("PcmFrames", kind_id(conduit_audio::AUDIO_PCM_INFO_ID))?;
    startup.insert_value_kind_alias("ChatMessage", kind_id(crate::CHAT_MESSAGE_VALUE_KIND))?;
    for contract in [
        speech_recognition_contract(),
        speech_clip_recognition_contract(),
        speech_recognition_to_text_contract(),
        streaming_speech_recognition_contract(),
        committed_recognition_turn_contract(),
        committed_turn_to_text_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert_kind(contract.into_semantic_capability_contract())
            .map_err(|error| error.to_string())?;
    }
    crate::install_speech_recognition_adapters(startup, profile)?;
    Ok(())
}

pub fn encode_speech_recognition_result(
    result: &SpeechRecognitionResult,
) -> Result<Vec<u8>, SpeechRecognitionValueError> {
    validate_result(result)?;
    let encoded = serde_json::to_vec(&SpeechRecognitionValue {
        schema: "conduit.speech/recognition-result-value@2".into(),
        result: result.clone().into(),
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
    if value.schema != "conduit.speech/recognition-result-value@2" {
        return Err(SpeechRecognitionValueError::InvalidValue);
    }
    let result = SpeechRecognitionResult::try_from(value.result)?;
    if encode_speech_recognition_result(&result)? != encoded {
        return Err(SpeechRecognitionValueError::NonCanonical);
    }
    Ok(result)
}

pub fn project_recognized_text(encoded: &[u8]) -> Result<Vec<u8>, RecognitionTextRefusal> {
    let result = decode_speech_recognition_result(encoded)
        .map_err(|_| RecognitionTextRefusal::InvalidResult)?;
    match result {
        SpeechRecognitionResult::Recognized(recognized) => {
            Ok(recognized.text().get().as_bytes().to_vec())
        }
        SpeechRecognitionResult::NoSpeech(_) => Err(RecognitionTextRefusal::NotRecognized),
    }
}

fn validate_result(result: &SpeechRecognitionResult) -> Result<(), SpeechRecognitionValueError> {
    result
        .clone()
        .into_structured()
        .map(|_| ())
        .map_err(|_| SpeechRecognitionValueError::InvalidValue)
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
            return Ok(SpeechRecognitionAttempt::Result(
                no_speech_result(
                    audio_sha256,
                    audio.len() as u32,
                    "tongues/recorded-fixture@1".into(),
                )
                .expect("validated recorded recognition result"),
            ));
        }
        if let Some(fixture) = self
            .fixtures
            .iter()
            .find(|fixture| fixture.audio_sha256 == audio_sha256)
        {
            return Ok(SpeechRecognitionAttempt::Result(
                recognized_result(
                    audio_sha256,
                    audio.len() as u32,
                    "tongues/recorded-fixture@1".into(),
                    fixture.transcript.clone(),
                )
                .expect("validated recorded recognition result"),
            ));
        }
        Ok(SpeechRecognitionAttempt::Failed(
            crate::SpeechRecognitionAudioDigest::new(audio_sha256)
                .expect("SHA-256 is exactly 32 bytes"),
        ))
    }

    pub fn resource_unavailable() -> SpeechRecognitionAttempt {
        SpeechRecognitionAttempt::ResourceUnavailable
    }
}

pub fn recognized_result(
    audio_sha256: [u8; 32],
    audio_extent_bytes: u32,
    provider_identity: String,
    text: String,
) -> Result<SpeechRecognitionResult, SpeechRecognitionValueError> {
    SpeechRecognitionResult::recognized(
        audio_extent_bytes,
        crate::SpeechRecognitionAudioDigest::new(audio_sha256)
            .map_err(|_| SpeechRecognitionValueError::InvalidValue)?,
        crate::SpeechRecognitionProviderIdentity::new(provider_identity)
            .map_err(|_| SpeechRecognitionValueError::InvalidValue)?,
        crate::RecognizedSpeechText::new(text)
            .map_err(|_| SpeechRecognitionValueError::InvalidValue)?,
    )
    .map_err(|_| SpeechRecognitionValueError::InvalidValue)
}

pub fn no_speech_result(
    audio_sha256: [u8; 32],
    audio_extent_bytes: u32,
    provider_identity: String,
) -> Result<SpeechRecognitionResult, SpeechRecognitionValueError> {
    SpeechRecognitionResult::no_speech(
        audio_extent_bytes,
        crate::SpeechRecognitionAudioDigest::new(audio_sha256)
            .map_err(|_| SpeechRecognitionValueError::InvalidValue)?,
        crate::SpeechRecognitionProviderIdentity::new(provider_identity)
            .map_err(|_| SpeechRecognitionValueError::InvalidValue)?,
    )
    .map_err(|_| SpeechRecognitionValueError::InvalidValue)
}

fn validate_audio(audio: &[u8]) -> Result<(PcmFrameHeader, &[u8]), SpeechRecognitionRefusal> {
    if audio.len() > MAXIMUM_RECOGNITION_AUDIO_BYTES {
        return Err(SpeechRecognitionRefusal::AudioTooLarge);
    }
    let (header, payload) =
        PcmFrameHeader::decode_frame(audio).map_err(|_| SpeechRecognitionRefusal::InvalidPcm)?;
    if header.representation() != PcmSampleRepresentation::Signed16LittleEndian
        || header.layout() != PcmChannelLayout::Mono
        || header.sample_rate_hz() != 16_000
        || header.discontinuity()
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
        abnormal_kind: None,
    }
}
