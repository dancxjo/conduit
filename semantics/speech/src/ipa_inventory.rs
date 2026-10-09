//! Exact inventory definitions bound to executed Unicode IPA parsing.
//! Preparation allocates; this is neither a Flow play path nor realization.
use crate::{ipa_admission::*, ipa_notation::*, semantic::*};
use alloc::{format, vec::Vec};
use conduit_language::LanguageVariety;

#[derive(Debug)]
pub enum IpaInventoryRefusal {
    Basis,
    BindingCoverage,
    BindingUnits,
    ForeignPhoneReference,
    UnknownPhoneme,
    AmbiguousPhoneme,
    UnsupportedPhonemeBinding,
    Notation(IpaNotationRefusal),
}

/// Borrows the whole supplied inventory and profile. Every definition retains
/// its explicit relation and original parsed transcription. Equal spellings do
/// not equate phones and phonemes or assert a realization correspondence.
pub struct PreparedIpaInventory<'a> {
    inventory: &'a SpeechInventory,
    basis: SpeechIpaInventoryNotationBasis,
    notation: PreparedIpaNotationProfile<'a>,
    phones: Vec<(SpeechIpaPhoneDefinitionBinding, AdmittedIpaTranscription)>,
    phonemes: Vec<(SpeechIpaPhonemeDefinitionBinding, AdmittedIpaTranscription)>,
}
/// One phoneme resolved against the complete explicit inventory. The original
/// definition and supplied notation basis survive beside the executed parsing.
pub struct AdmittedPhonemeNotation<'a> {
    inventory: &'a SpeechInventory,
    definition: &'a SpeechPhoneme,
    basis: SpeechIpaInventoryNotationBasis,
    notation: AdmittedIpaTranscription,
}
impl AdmittedPhonemeNotation<'_> {
    pub fn inventory(&self) -> &SpeechInventory {
        self.inventory
    }
    pub fn definition(&self) -> &SpeechPhoneme {
        self.definition
    }
    pub fn basis(&self) -> &SpeechIpaInventoryNotationBasis {
        &self.basis
    }
    pub fn notation(&self) -> &AdmittedIpaTranscription {
        &self.notation
    }
}

impl<'a> PreparedIpaInventory<'a> {
    /// Qualified phonemic entrance: identical IPA in another inventory is not
    /// this phoneme, and two matching definitions refuse rather than guessing.
    pub fn phoneme_from_ipa(
        &self,
        spelling: alloc::string::String,
        provenance: SpeechEvidenceProvenance,
    ) -> Result<AdmittedPhonemeNotation<'a>, IpaInventoryRefusal> {
        use IpaInventoryRefusal::*;
        if spelling.len() > 64 {
            return Err(Notation(IpaNotationRefusal::Capacity));
        }
        let notation = admit_ipa_transcription(
            &self.notation,
            format!("/{spelling}/"),
            SpeechIpaDisplayKind::Phonemic,
            provenance,
        )
        .map_err(Notation)?;
        let mut matches = self
            .phonemes
            .iter()
            .filter(|(binding, _)| matches_units(&notation, binding.units().as_slice()));
        let (binding, _) = matches.next().ok_or(UnknownPhoneme)?;
        if matches.next().is_some() {
            return Err(AmbiguousPhoneme);
        }
        let definition = self
            .inventory
            .phonemes()
            .as_slice()
            .iter()
            .find(|definition| definition.identity() == binding.phoneme())
            .ok_or(UnknownPhoneme)?;
        Ok(AdmittedPhonemeNotation {
            inventory: self.inventory,
            definition,
            basis: self.basis.clone(),
            notation,
        })
    }

    pub fn basis(&self) -> &SpeechIpaInventoryNotationBasis {
        &self.basis
    }
    pub fn inventory(&self) -> &'a SpeechInventory {
        self.inventory
    }
    pub fn notation(&self) -> &PreparedIpaNotationProfile<'a> {
        &self.notation
    }
    pub fn phones(&self) -> &[(SpeechIpaPhoneDefinitionBinding, AdmittedIpaTranscription)] {
        &self.phones
    }
    pub fn phonemes(&self) -> &[(SpeechIpaPhonemeDefinitionBinding, AdmittedIpaTranscription)] {
        &self.phonemes
    }
    pub fn prepare(
        inventory: &'a SpeechInventory,
        profile: &'a SpeechIpaNotationProfile,
        variety: &LanguageVariety,
        revision: &SpeechSegmentRevisionId,
        phone_bindings: &[SpeechIpaPhoneDefinitionBinding],
        phoneme_bindings: &[SpeechIpaPhonemeDefinitionBinding],
    ) -> Result<Self, IpaInventoryRefusal> {
        use IpaInventoryRefusal::*;
        let basis = SpeechIpaInventoryNotationBasis::new(
            inventory.identity().clone(),
            inventory.language().clone(),
            profile.clone(),
            revision.clone(),
            variety.clone(),
        )
        .map_err(|_| Basis)?;
        if phone_bindings.len() != inventory.phones().len()
            || phoneme_bindings.len() != inventory.phonemes().len()
        {
            return Err(BindingCoverage);
        }
        for phoneme in inventory.phonemes().as_slice() {
            let contains = |id: &PhoneId| {
                inventory
                    .phones()
                    .as_slice()
                    .iter()
                    .any(|phone| phone.identity() == id)
            };
            if phoneme
                .default_phone()
                .as_ref()
                .is_some_and(|id| !contains(id))
                || phoneme
                    .possible_phones()
                    .as_slice()
                    .iter()
                    .any(|id| !contains(id))
                || phoneme
                    .allophones()
                    .as_slice()
                    .iter()
                    .any(|rule| !contains(rule.phone()))
            {
                return Err(ForeignPhoneReference);
            }
        }
        let notation = PreparedIpaNotationProfile::prepare(profile).map_err(Notation)?;
        let mut phones = Vec::new();
        for (index, definition) in inventory.phones().as_slice().iter().enumerate() {
            if inventory.phones().as_slice()[..index]
                .iter()
                .any(|prior| prior.identity() == definition.identity())
            {
                return Err(BindingCoverage);
            }
            let mut bindings = phone_bindings
                .iter()
                .filter(|binding| binding.phone() == definition.identity());
            let binding = bindings.next().ok_or(BindingCoverage)?;
            if bindings.next().is_some() {
                return Err(BindingCoverage);
            }
            let parsed = admit_ipa_transcription(
                &notation,
                format!("[{}]", definition.ipa()),
                SpeechIpaDisplayKind::Phonetic,
                binding.provenance().clone(),
            )
            .map_err(Notation)?;
            if !matches_units(&parsed, binding.units().as_slice()) {
                return Err(BindingUnits);
            }
            phones.push((binding.clone(), parsed));
        }
        let mut phonemes = Vec::new();
        for (index, definition) in inventory.phonemes().as_slice().iter().enumerate() {
            if inventory.phonemes().as_slice()[..index]
                .iter()
                .any(|prior| prior.identity() == definition.identity())
            {
                return Err(BindingCoverage);
            }
            let mut bindings = phoneme_bindings
                .iter()
                .filter(|binding| binding.phoneme() == definition.identity());
            let binding = bindings.next().ok_or(BindingCoverage)?;
            if bindings.next().is_some() {
                return Err(BindingCoverage);
            }
            let parsed = admit_ipa_transcription(
                &notation,
                format!("/{}/", definition.notation()),
                SpeechIpaDisplayKind::Phonemic,
                binding.provenance().clone(),
            )
            .map_err(Notation)?;
            if !matches_units(&parsed, binding.units().as_slice()) {
                return Err(BindingUnits);
            }
            // Stress and boundaries belong to a transcription, not a phoneme
            // identity. Admit this once for both single and full constructors.
            if binding.units().as_slice().iter().any(|id| {
                profile.units().as_slice().iter().any(|unit| {
                    unit.identity() == id
                        && !matches!(
                            unit.kind(),
                            SpeechIpaUnitKind::Segment | SpeechIpaUnitKind::Length
                        )
                })
            }) {
                return Err(UnsupportedPhonemeBinding);
            }
            phonemes.push((binding.clone(), parsed));
        }
        Ok(Self {
            inventory,
            basis,
            notation,
            phones,
            phonemes,
        })
    }
}
fn matches_units(parsed: &AdmittedIpaTranscription, expected: &[SpeechIpaUnitId]) -> bool {
    let actual = parsed.transcription().units().as_slice();
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.unit() == expected)
}
