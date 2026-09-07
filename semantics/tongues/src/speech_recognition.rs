//! Portable bounded speech-recognition meaning and one exact fixture adapter.

use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, AUDIO_PCM_INFO_ID};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use sha2::{Digest, Sha256};
use std::{string::String, vec, vec::Vec};

pub const SPEECH_RECOGNIZE_KIND: &str = "speech/recognize";
pub const SPEECH_RECOGNIZE_REVISION: &str = "conduit.speech/recognize@1";
pub const SPEECH_RECOGNITION_RESULT_KIND: &str = "speech/recognition-result@1";
pub const MAXIMUM_RECOGNIZED_TEXT_BYTES: usize = 256;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpeechRecognitionDisposition {
    Recognized,
    NoSpeech,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

pub fn install_speech_recognition_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let contract = speech_recognition_contract();
    startup.insert(KindSignature {
        kind: SPEECH_RECOGNIZE_KIND.into(),
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
        .map_err(|error| error.to_string())
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
