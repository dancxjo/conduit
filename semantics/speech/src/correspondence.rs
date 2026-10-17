//! Checked finite correspondence over exact supplied phoneme and phone tapes.
//! No pronunciation inference, acoustic projection, or commitment is performed.
use crate::{ipa_inventory::PreparedIpaInventory, semantic::*};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum CorrespondenceRefusal {
    Native(NativeBindingRefusal),
    Basis,
    Source,
    Duplicate,
    Missing,
    Unresolved,
    InventoryMember,
    ContradictoryNestedRealization,
}

/// Borrows the original owners, including executed full IPA inventory admission.
/// Decoding the component never creates this receipt; preparation must run again.
pub struct PreparedSpeechCorrespondence<'a, 'inventory> {
    intent: &'a SpeechUtteranceIntent,
    phonemes: &'a SpeechPhonemeSequence,
    phones: &'a SpeechPhoneSequence,
    text: &'a LanguageText,
    alignment: &'a SpeechRealizationAlignment,
    inventory: &'a PreparedIpaInventory<'inventory>,
}
impl<'a, 'inventory> PreparedSpeechCorrespondence<'a, 'inventory> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn phonemes(&self) -> &'a SpeechPhonemeSequence {
        self.phonemes
    }
    pub fn phones(&self) -> &'a SpeechPhoneSequence {
        self.phones
    }
    pub fn text(&self) -> &'a LanguageText {
        self.text
    }
    pub fn alignment(&self) -> &'a SpeechRealizationAlignment {
        self.alignment
    }
    pub fn inventory(&self) -> &'a PreparedIpaInventory<'inventory> {
        self.inventory
    }

    pub fn prepare(
        intent: &'a SpeechUtteranceIntent,
        phonemes: &'a SpeechPhonemeSequence,
        phones: &'a SpeechPhoneSequence,
        text: &'a LanguageText,
        alignment: &'a SpeechRealizationAlignment,
        inventory: &'a PreparedIpaInventory<'inventory>,
    ) -> Result<Self, CorrespondenceRefusal> {
        use CorrespondenceRefusal::*;
        if alignment.inventory_basis() != inventory.basis()
            || alignment.phoneme_basis() != phonemes.basis()
            || alignment.phone_basis() != phones.basis()
            || phones.basis().utterance_id() != intent.utterance_id()
            || phones.basis().revision_id() != intent.revision_id()
            || phones.basis().inventory_id() != intent.inventory_id()
            || phones.basis().language() != intent.language()
            || inventory.inventory().identity() != intent.inventory_id()
            || inventory.inventory().language() != intent.language()
        {
            return Err(Basis);
        }
        let count = u32::try_from(text.text().chars().count()).map_err(|_| Source)?;
        LanguageTextReferenceMatch::new(text.clone(), alignment.source().clone(), count)
            .map_err(Native)?;
        if *alignment.source().range().start() != 0 || *alignment.source().range().end() != count {
            return Err(Source);
        }
        let mut seen_phonemes = [false; 256];
        let mut seen_phones = [false; 256];
        for correspondence in alignment.correspondences().as_slice() {
            let (left, right) = match correspondence {
                SpeechRealizationCorrespondence::Aligned(value) => {
                    (value.phonemes().as_slice(), value.phones().as_slice())
                }
                SpeechRealizationCorrespondence::Deleted(value) => {
                    (value.phonemes().as_slice(), &[][..])
                }
                SpeechRealizationCorrespondence::Inserted(value) => {
                    (&[][..], value.phones().as_slice())
                }
            };
            // Legacy nested copies may be absent. If supplied, they must agree
            // with this explicit occurrence relation, never replace its IDs.
            for reference in left {
                let index = member(reference, phonemes.basis(), phonemes.tokens().len())?;
                let nested = phonemes.tokens()[index].realized_as().as_slice();
                if !nested.is_empty() {
                    if nested.len() != right.len() {
                        return Err(ContradictoryNestedRealization);
                    }
                    for (copy, target) in nested.iter().zip(right) {
                        let target = member(target, phones.basis(), phones.tokens().len())?;
                        if copy != &phones.tokens()[target] {
                            return Err(ContradictoryNestedRealization);
                        }
                    }
                }
            }
            for reference in left {
                let index = member(reference, phonemes.basis(), phonemes.tokens().len())?;
                if core::mem::replace(&mut seen_phonemes[index], true) {
                    return Err(Duplicate);
                }
                let PhonemeSpecification::Known(id) = phonemes.tokens()[index].phoneme() else {
                    return Err(Unresolved);
                };
                if !inventory
                    .inventory()
                    .phonemes()
                    .as_slice()
                    .iter()
                    .any(|definition| definition.identity() == id)
                {
                    return Err(InventoryMember);
                }
            }
            for reference in right {
                let index = member(reference, phones.basis(), phones.tokens().len())?;
                if core::mem::replace(&mut seen_phones[index], true) {
                    return Err(Duplicate);
                }
                let PhoneSpecification::Known(id) = phones.tokens()[index].phone() else {
                    return Err(Unresolved);
                };
                if !inventory
                    .inventory()
                    .phones()
                    .as_slice()
                    .iter()
                    .any(|definition| definition.identity() == id)
                {
                    return Err(InventoryMember);
                }
            }
        }
        if seen_phonemes[..phonemes.tokens().len()]
            .iter()
            .any(|seen| !seen)
            || seen_phones[..phones.tokens().len()]
                .iter()
                .any(|seen| !seen)
        {
            return Err(Missing);
        }
        let mut seen_events = [false; 256];
        for (event_index, event) in intent.events().as_slice().iter().enumerate() {
            let SpeechUtteranceIntentEvent::Segment(segment) = event else {
                continue;
            };
            let index = member(segment.occurrence(), phones.basis(), phones.tokens().len())?;
            if core::mem::replace(&mut seen_events[index], true) {
                return Err(Duplicate);
            }
            SpeechRealizationIntentPhone::new(
                event_index as u64,
                intent.clone(),
                segment.occurrence().clone(),
                phones.tokens()[index].phone().clone(),
            )
            .map_err(Native)?;
            // A multi-phoneme relation has no scalar phoneme value on this phone.
            // Alternatives would falsely mean uncertainty rather than conjunction.
            let group = alignment
                .correspondences()
                .as_slice()
                .iter()
                .find_map(|group| match group {
                    SpeechRealizationCorrespondence::Aligned(value)
                        if value.phones().as_slice().contains(segment.occurrence()) =>
                    {
                        Some(Some(value))
                    }
                    SpeechRealizationCorrespondence::Inserted(value)
                        if value.phones().as_slice().contains(segment.occurrence()) =>
                    {
                        Some(None)
                    }
                    _ => None,
                })
                .ok_or(Missing)?;
            match group {
                Some(group) if group.phonemes().len() == 1 => {
                    let index = *group.phonemes()[0].ordinal() as usize;
                    if segment.phoneme() != phonemes.tokens()[index].phoneme() {
                        return Err(Basis);
                    }
                }
                _ if matches!(segment.phoneme(), PhonemeSpecification::NotApplicable) => {}
                _ => return Err(Basis),
            }
        }
        if seen_events[..phones.tokens().len()]
            .iter()
            .any(|seen| !seen)
        {
            return Err(Missing);
        }
        Ok(Self {
            intent,
            phonemes,
            phones,
            text,
            alignment,
            inventory,
        })
    }
}

fn member(
    reference: &LanguageSpeechTokenRef,
    basis: &SpeechTokenSequenceBasis,
    count: usize,
) -> Result<usize, CorrespondenceRefusal> {
    use CorrespondenceRefusal::*;
    let matched =
        SpeechSequenceBasisMatch::new(basis.clone(), reference.clone()).map_err(Native)?;
    SpeechSequenceReferenceMatch::new(matched, count as u32).map_err(Native)?;
    Ok(*reference.ordinal() as usize)
}
