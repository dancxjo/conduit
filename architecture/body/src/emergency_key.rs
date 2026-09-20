//! Durable finite acoustic-emergency configuration and sequence matching.

use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const EMERGENCY_VOCABULARY_VERSION: &str = "conduit.emergency-words/en-us@1";
pub const EMERGENCY_WORDS: [&str; 16] = [
    "copper", "kestrel", "lantern", "marble", "orchid", "pioneer", "quartz", "raven", "saffron",
    "timber", "velvet", "willow", "zephyr", "badger", "comet", "harbor",
];
pub const MAX_EMERGENCY_WORD_GAP_MILLIS: u32 = 2_000;
pub const MAX_EMERGENCY_CONFIGURATION_REVISIONS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmergencyKey {
    pub vocabulary_version: String,
    pub word_ids: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergencyKeyRefusal {
    EntropyUnavailable,
    InvalidEntropy,
    DuplicateWord,
    InvalidVocabulary,
}

impl EmergencyKey {
    /// Select three distinct words from admitted entropy supplied by the Host.
    /// There is deliberately no clock, identity, or deterministic fallback.
    pub fn from_admitted_entropy(entropy: [u8; 3]) -> Result<Self, EmergencyKeyRefusal> {
        let count = EMERGENCY_WORDS.len() as u8;
        let word_ids = [entropy[0] % count, entropy[1] % count, entropy[2] % count];
        if word_ids[0] == word_ids[1] || word_ids[0] == word_ids[2] || word_ids[1] == word_ids[2] {
            return Err(EmergencyKeyRefusal::DuplicateWord);
        }
        Ok(Self {
            vocabulary_version: EMERGENCY_VOCABULARY_VERSION.into(),
            word_ids,
        })
    }

    pub fn validate(&self) -> Result<(), EmergencyKeyRefusal> {
        if self.vocabulary_version != EMERGENCY_VOCABULARY_VERSION
            || self
                .word_ids
                .iter()
                .any(|id| usize::from(*id) >= EMERGENCY_WORDS.len())
        {
            return Err(EmergencyKeyRefusal::InvalidVocabulary);
        }
        if self.word_ids[0] == self.word_ids[1]
            || self.word_ids[0] == self.word_ids[2]
            || self.word_ids[1] == self.word_ids[2]
        {
            return Err(EmergencyKeyRefusal::DuplicateWord);
        }
        Ok(())
    }

    pub fn words(&self) -> Result<[&'static str; 3], EmergencyKeyRefusal> {
        self.validate()?;
        Ok([
            EMERGENCY_WORDS[usize::from(self.word_ids[0])],
            EMERGENCY_WORDS[usize::from(self.word_ids[1])],
            EMERGENCY_WORDS[usize::from(self.word_ids[2])],
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmergencyAcousticAvailability {
    Ready,
    PhysicallyMuted,
    ProviderUnavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableEmergencyConfiguration {
    pub revision: u64,
    pub key: EmergencyKey,
    pub detector_version: String,
    pub entropy_provider_id: String,
    pub acoustic_availability: EmergencyAcousticAvailability,
}

impl DurableEmergencyConfiguration {
    pub fn validate(&self) -> Result<(), EmergencyKeyRefusal> {
        self.key.validate()?;
        if self.revision == 0
            || self.detector_version.is_empty()
            || self.detector_version.len() > 96
            || self.entropy_provider_id.is_empty()
            || self.entropy_provider_id.len() > 96
        {
            return Err(EmergencyKeyRefusal::InvalidVocabulary);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcousticObservation {
    Word { word_id: u8, ended_at_millis: u64 },
    InvalidConfidence,
    Overflow,
    MicrophoneLost,
    StaleGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcousticDecision {
    Waiting,
    Advanced,
    Triggered,
    Reset,
    Unavailable,
    Suppressed,
}

/// Fixed-state matcher downstream of a finite vocabulary detector. Any wrong
/// word or invalid input resets progress; a successful phrase is one-shot.
pub struct EmergencySequenceMatcher {
    key: EmergencyKey,
    matched: u8,
    last_word_at: u64,
    triggered: bool,
}

impl EmergencySequenceMatcher {
    pub fn new(key: EmergencyKey) -> Result<Self, EmergencyKeyRefusal> {
        key.validate()?;
        Ok(Self {
            key,
            matched: 0,
            last_word_at: 0,
            triggered: false,
        })
    }

    pub fn observe(&mut self, observation: AcousticObservation) -> AcousticDecision {
        if self.triggered {
            return AcousticDecision::Suppressed;
        }
        let AcousticObservation::Word {
            word_id,
            ended_at_millis,
        } = observation
        else {
            self.reset();
            return match observation {
                AcousticObservation::MicrophoneLost | AcousticObservation::StaleGeneration => {
                    AcousticDecision::Unavailable
                }
                _ => AcousticDecision::Reset,
            };
        };
        if self.matched > 0
            && (ended_at_millis <= self.last_word_at
                || ended_at_millis - self.last_word_at > u64::from(MAX_EMERGENCY_WORD_GAP_MILLIS))
        {
            self.reset();
        }
        let expected = self.key.word_ids[usize::from(self.matched)];
        if word_id != expected {
            self.reset();
            return AcousticDecision::Reset;
        }
        self.matched += 1;
        self.last_word_at = ended_at_millis;
        if self.matched == 3 {
            self.triggered = true;
            AcousticDecision::Triggered
        } else {
            AcousticDecision::Advanced
        }
    }

    fn reset(&mut self) {
        self.matched = 0;
        self.last_word_at = 0;
    }
}
