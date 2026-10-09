//! Universal single-phone notation, independent of language inventory membership.
use crate::{
    ipa_notation::IpaNotationRefusal,
    ipa_unicode::{supported_unit, UnitKind},
    semantic::*,
};
use alloc::string::String;

/// A checked single phonetic segment. No language, phoneme or inventory is
/// inferred. The spelling stays exact, including combining marks and tie bars.
pub struct AdmittedPhoneNotation {
    notation: SpeechPhoneNotation,
}
impl AdmittedPhoneNotation {
    pub fn notation(&self) -> &SpeechPhoneNotation {
        &self.notation
    }
}

/// Qualified quoted-IPA entrance; unlike phoneme resolution, this operation
/// needs no language/variety inventory. Version1 supports a finite IPA subset.
pub fn phone_from_ipa(
    spelling: String,
    provenance: SpeechEvidenceProvenance,
) -> Result<AdmittedPhoneNotation, IpaNotationRefusal> {
    if !supported_unit(&spelling, UnitKind::Segment) {
        return Err(IpaNotationRefusal::UnsupportedSpelling);
    }
    let spelling = SpeechIpaSpelling::new(spelling).map_err(IpaNotationRefusal::Native)?;
    let notation =
        SpeechPhoneNotation::new(provenance, spelling).map_err(IpaNotationRefusal::Native)?;
    Ok(AdmittedPhoneNotation { notation })
}
