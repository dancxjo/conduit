//! Join actual executed IPA inventory admission to the original shared owner.
use crate::{
    ipa_inventory::PreparedIpaInventory, semantic::*, shared_intent::PreparedSpeechUtteranceIntent,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum SharedIpaRefusal {
    Native(NativeBindingRefusal),
    MissingPhone,
    MissingPhoneme,
    AmbiguousDefinition,
    Representation,
}
pub struct SharedPhoneMembership<'a> {
    ordinal: u32,
    candidate: SpeechPhoneInventoryCandidate,
    definition: &'a SpeechPhone,
    identity: SpeechPhoneDefinitionMatch,
}
impl<'a> SharedPhoneMembership<'a> {
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub fn candidate(&self) -> &SpeechPhoneInventoryCandidate {
        &self.candidate
    }
    pub fn definition(&self) -> &'a SpeechPhone {
        self.definition
    }
    pub fn identity(&self) -> &SpeechPhoneDefinitionMatch {
        &self.identity
    }
}
pub struct SharedPhonemeMembership<'a> {
    ordinal: u32,
    candidate: SpeechPhonemeInventoryCandidate,
    definition: &'a SpeechPhoneme,
    identity: SpeechPhonemeDefinitionMatch,
}
impl<'a> SharedPhonemeMembership<'a> {
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub fn candidate(&self) -> &SpeechPhonemeInventoryCandidate {
        &self.candidate
    }
    pub fn definition(&self) -> &'a SpeechPhoneme {
        self.definition
    }
    pub fn identity(&self) -> &SpeechPhonemeDefinitionMatch {
        &self.identity
    }
}
pub struct PreparedIpaSpeechUtteranceIntent<'a, 'material> {
    shared: &'a PreparedSpeechUtteranceIntent<'material>,
    ipa: &'a PreparedIpaInventory<'material>,
    basis: SpeechSharedIntentIpaBasis,
    phones: Vec<SharedPhoneMembership<'material>>,
    phonemes: Vec<SharedPhonemeMembership<'material>>,
}
impl<'a, 'material> PreparedIpaSpeechUtteranceIntent<'a, 'material> {
    pub fn shared(&self) -> &'a PreparedSpeechUtteranceIntent<'material> {
        self.shared
    }
    pub fn ipa(&self) -> &'a PreparedIpaInventory<'material> {
        self.ipa
    }
    pub fn basis(&self) -> &SpeechSharedIntentIpaBasis {
        &self.basis
    }
    pub fn phones(&self) -> &[SharedPhoneMembership<'material>] {
        &self.phones
    }
    pub fn phonemes(&self) -> &[SharedPhonemeMembership<'material>] {
        &self.phonemes
    }
    pub fn prepare(
        shared: &'a PreparedSpeechUtteranceIntent<'material>,
        ipa: &'a PreparedIpaInventory<'material>,
    ) -> Result<Self, SharedIpaRefusal> {
        use SharedIpaRefusal::*;
        let original = shared.original();
        let basis = SpeechSharedIntentIpaBasis::new(
            shared.components().context.variety().clone(),
            original.inventory_id().clone(),
            original.language().clone(),
            original.revision_id().clone(),
            ipa.basis().clone(),
        )
        .map_err(Native)?;
        let inventory = ipa.inventory();
        let mut phones = Vec::new();
        let mut phonemes = Vec::new();
        for (ordinal, token) in shared
            .components()
            .phones
            .tokens()
            .as_slice()
            .iter()
            .enumerate()
        {
            let ordinal = u32::try_from(ordinal).map_err(|_| Representation)?;
            let mut admit = |index: usize, requested: &PhoneId| -> Result<(), SharedIpaRefusal> {
                let candidate = SpeechPhoneInventoryCandidate::new(
                    index as u64,
                    requested.clone(),
                    token.clone(),
                )
                .map_err(Native)?;
                let mut found = inventory
                    .phones()
                    .as_slice()
                    .iter()
                    .filter(|d| d.identity() == requested);
                let definition = found.next().ok_or(MissingPhone)?;
                if found.next().is_some() {
                    return Err(AmbiguousDefinition);
                }
                let identity = SpeechPhoneDefinitionMatch::new(
                    definition.identity().clone(),
                    requested.clone(),
                )
                .map_err(Native)?;
                phones.push(SharedPhoneMembership {
                    ordinal,
                    candidate,
                    definition,
                    identity,
                });
                Ok(())
            };
            match token.phone() {
                PhoneSpecification::Known(id) => admit(0, id)?,
                PhoneSpecification::Variable(ids) => {
                    for (index, id) in ids.as_slice().iter().enumerate() {
                        admit(index, id)?;
                    }
                }
                PhoneSpecification::Gradient(value) => admit(0, value.value())?,
                PhoneSpecification::Unknown
                | PhoneSpecification::Unspecified
                | PhoneSpecification::NotApplicable => {}
            }
        }
        for (ordinal, token) in shared
            .components()
            .phonemes
            .tokens()
            .as_slice()
            .iter()
            .enumerate()
        {
            let ordinal = u32::try_from(ordinal).map_err(|_| Representation)?;
            let mut admit = |index: usize, requested: &PhonemeId| -> Result<(), SharedIpaRefusal> {
                let candidate = SpeechPhonemeInventoryCandidate::new(
                    index as u64,
                    requested.clone(),
                    token.clone(),
                )
                .map_err(Native)?;
                let mut found = inventory
                    .phonemes()
                    .as_slice()
                    .iter()
                    .filter(|d| d.identity() == requested);
                let definition = found.next().ok_or(MissingPhoneme)?;
                if found.next().is_some() {
                    return Err(AmbiguousDefinition);
                }
                let identity = SpeechPhonemeDefinitionMatch::new(
                    definition.identity().clone(),
                    requested.clone(),
                )
                .map_err(Native)?;
                phonemes.push(SharedPhonemeMembership {
                    ordinal,
                    candidate,
                    definition,
                    identity,
                });
                Ok(())
            };
            match token.phoneme() {
                PhonemeSpecification::Known(id) => admit(0, id)?,
                PhonemeSpecification::Variable(ids) => {
                    for (index, id) in ids.as_slice().iter().enumerate() {
                        admit(index, id)?;
                    }
                }
                PhonemeSpecification::Gradient(value) => admit(0, value.value())?,
                PhonemeSpecification::Unknown
                | PhonemeSpecification::Unspecified
                | PhonemeSpecification::NotApplicable => {}
            }
        }
        Ok(Self {
            shared,
            ipa,
            basis,
            phones,
            phonemes,
        })
    }
}
