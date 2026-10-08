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
impl<'a> PreparedIpaInventory<'a> {
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
