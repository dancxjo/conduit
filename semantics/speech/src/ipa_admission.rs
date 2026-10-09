//! Opaque executed notation receipt. A freely constructible Native profile-match
//! record alone cannot establish that Unicode/profile parsing actually ran.
use crate::{
    ipa_notation::{IpaNotationRefusal, PreparedIpaNotationProfile},
    semantic::*,
};

/// Retains the complete original transcription and supplied profile, not IDs or
/// display IPA reconstructed into phonological authority. No phone/phoneme
/// membership, commitment, terminal projection or playback is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpaProfileMismatch;

pub struct AdmittedIpaTranscription {
    checked: SpeechIpaProfileMatch,
}
impl AdmittedIpaTranscription {
    /// Compare the complete profile, including definitions, aliases and evidence.
    /// Matching inventory/profile IDs alone do not establish this correlation.
    pub fn require_profile(
        &self,
        expected: &SpeechIpaNotationProfile,
    ) -> Result<(), IpaProfileMismatch> {
        if self.profile() == expected {
            Ok(())
        } else {
            Err(IpaProfileMismatch)
        }
    }
    pub fn checked_match(&self) -> &SpeechIpaProfileMatch {
        &self.checked
    }
    pub fn transcription(&self) -> &SpeechIpaTranscription {
        self.checked.transcription()
    }
    pub fn profile(&self) -> &SpeechIpaNotationProfile {
        self.checked.profile()
    }
}

/// The only entrance to this receipt executes the prepared profile's complete
/// partition/syntax checks and its exact Native profile correlation laws.
pub fn admit_ipa_transcription(
    profile: &PreparedIpaNotationProfile<'_>,
    original: alloc::string::String,
    display: SpeechIpaDisplayKind,
    provenance: SpeechEvidenceProvenance,
) -> Result<AdmittedIpaTranscription, IpaNotationRefusal> {
    profile
        .parse(original, display, provenance)
        .map(|checked| AdmittedIpaTranscription { checked })
}
