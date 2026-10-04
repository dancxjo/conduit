//! Declared phoneme/phone correspondence, without evaluating allophone contexts.
//! All compatible declarations are retained; this neither chooses a declaration
//! nor establishes contextual eligibility, source resolution or commitment.
use crate::{
    intent_inventory::{
        resolve_intent_inventory_phone, IntentInventoryRefusal, ResolvedIntentPhone,
    },
    semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum DeclaredRealizationReason {
    Phone(IntentInventoryRefusal),
    UnresolvedPhoneme(PhonemeSpecification),
    MissingPhonemeDefinition,
    AmbiguousPhonemeDefinition,
    Native(NativeBindingRefusal),
    UndeclaredPhone,
}
#[derive(Debug)]
pub struct DeclaredRealizationRefusal {
    pub event: usize,
    pub reason: DeclaredRealizationReason,
}

/// Original declarations retain their status, confidence, environments and
/// conditions. A possible phone is not an unconditional allophone rule.
pub enum PhoneDeclaration<'a> {
    Default(&'a PhoneId),
    Possible {
        index: usize,
        phone: &'a PhoneId,
    },
    Allophone {
        index: usize,
        declaration: &'a SpeechPhonemeAllophone,
    },
}
impl PhoneDeclaration<'_> {
    fn phone(&self) -> &PhoneId {
        match self {
            Self::Default(phone) | Self::Possible { phone, .. } => phone,
            Self::Allophone { declaration, .. } => declaration.phone(),
        }
    }
}
pub struct PhoneDeclarationReceipt<'a> {
    declaration: PhoneDeclaration<'a>,
    checked: SpeechPhoneDefinitionMatch,
}
impl<'a> PhoneDeclarationReceipt<'a> {
    pub fn declaration(&self) -> &PhoneDeclaration<'a> {
        &self.declaration
    }
    pub fn checked_phone(&self) -> &SpeechPhoneDefinitionMatch {
        &self.checked
    }
}
/// At most one default, eight possible phones and eight allophone declarations.
/// Preparation grows only within these native bounds and plays no PCM.
pub struct DeclaredIntentRealization<'a> {
    source: ResolvedIntentPhone<'a>,
    phoneme: &'a SpeechPhoneme,
    checked: SpeechPhonemeDefinitionMatch,
    declarations: Vec<PhoneDeclarationReceipt<'a>>,
}
impl<'a> DeclaredIntentRealization<'a> {
    pub fn source(&self) -> &ResolvedIntentPhone<'a> {
        &self.source
    }
    pub fn phoneme(&self) -> &'a SpeechPhoneme {
        self.phoneme
    }
    pub fn checked_phoneme(&self) -> &SpeechPhonemeDefinitionMatch {
        &self.checked
    }
    pub fn declarations(&self) -> &[PhoneDeclarationReceipt<'a>] {
        &self.declarations
    }
}

pub fn resolve_declared_intent_realization<'a>(
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    inventory: &'a SpeechInventory,
) -> Result<DeclaredIntentRealization<'a>, DeclaredRealizationRefusal> {
    resolve(intent, event, inventory).map_err(|reason| DeclaredRealizationRefusal { event, reason })
}
fn resolve<'a>(
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    inventory: &'a SpeechInventory,
) -> Result<DeclaredIntentRealization<'a>, DeclaredRealizationReason> {
    let source = resolve_intent_inventory_phone(intent, event, inventory)
        .map_err(DeclaredRealizationReason::Phone)?;
    let requested = match source.segment().phoneme() {
        PhonemeSpecification::Known(identity) => identity,
        state => return Err(DeclaredRealizationReason::UnresolvedPhoneme(state.clone())),
    };
    let mut matches = inventory
        .phonemes()
        .as_slice()
        .iter()
        .filter(|definition| definition.identity() == requested);
    let phoneme = matches
        .next()
        .ok_or(DeclaredRealizationReason::MissingPhonemeDefinition)?;
    if matches.next().is_some() {
        return Err(DeclaredRealizationReason::AmbiguousPhonemeDefinition);
    }
    let checked = SpeechPhonemeDefinitionMatch::new(phoneme.identity().clone(), requested.clone())
        .map_err(DeclaredRealizationReason::Native)?;
    let selected = source.definition().identity();
    let mut declarations = Vec::new();
    let candidates = phoneme
        .default_phone()
        .iter()
        .map(PhoneDeclaration::Default)
        .chain(
            phoneme
                .possible_phones()
                .as_slice()
                .iter()
                .enumerate()
                .map(|(index, phone)| PhoneDeclaration::Possible { index, phone }),
        )
        .chain(
            phoneme
                .allophones()
                .as_slice()
                .iter()
                .enumerate()
                .map(|(index, declaration)| PhoneDeclaration::Allophone { index, declaration }),
        );
    for declaration in candidates {
        if declaration.phone() == selected {
            let checked =
                SpeechPhoneDefinitionMatch::new(selected.clone(), declaration.phone().clone())
                    .map_err(DeclaredRealizationReason::Native)?;
            declarations.push(PhoneDeclarationReceipt {
                declaration,
                checked,
            });
        }
    }
    if declarations.is_empty() {
        return Err(DeclaredRealizationReason::UndeclaredPhone);
    }
    Ok(DeclaredIntentRealization {
        source,
        phoneme,
        checked,
        declarations,
    })
}
