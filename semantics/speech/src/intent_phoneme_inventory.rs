//! Exact original requested-phoneme membership, without inferred token features.
use crate::{
    occurrence_context::{
        resolve_intent_occurrence_context, IntentOccurrenceContext, OccurrenceContextRefusal,
    },
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum IntentPhonemeRefusal<'a> {
    Occurrence(OccurrenceContextRefusal),
    Basis(NativeBindingRefusal),
    Unresolved(&'a PhonemeSpecification),
    MissingDefinition,
    AmbiguousDefinition,
    Definition(NativeBindingRefusal),
}
pub struct ResolvedIntentPhoneme<'a> {
    occurrence: IntentOccurrenceContext<'a>,
    inventory: &'a SpeechInventory,
    definition: &'a SpeechPhoneme,
    basis: SpeechIntentInventoryBasis,
    identity: SpeechPhonemeDefinitionMatch,
}
impl<'a> ResolvedIntentPhoneme<'a> {
    pub fn occurrence(&self) -> &IntentOccurrenceContext<'a> {
        &self.occurrence
    }
    pub fn inventory(&self) -> &'a SpeechInventory {
        self.inventory
    }
    pub fn definition(&self) -> &'a SpeechPhoneme {
        self.definition
    }
    pub fn checked_basis(&self) -> &SpeechIntentInventoryBasis {
        &self.basis
    }
    pub fn checked_identity(&self) -> &SpeechPhonemeDefinitionMatch {
        &self.identity
    }
}
/// A Known symbol must name one exact definition in the supplied inventory.
/// Other states are retained as unresolved, without normalization or fallback.
pub fn resolve_intent_inventory_phoneme<'a>(
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    inventory: &'a SpeechInventory,
) -> Result<ResolvedIntentPhoneme<'a>, IntentPhonemeRefusal<'a>> {
    let occurrence = resolve_intent_occurrence_context(intent, event)
        .map_err(IntentPhonemeRefusal::Occurrence)?;
    let basis = SpeechIntentInventoryBasis::new(
        intent.inventory_id().clone(),
        intent.language().clone(),
        inventory.identity().clone(),
        inventory.language().clone(),
    )
    .map_err(IntentPhonemeRefusal::Basis)?;
    let requested = match occurrence.segment().phoneme() {
        PhonemeSpecification::Known(id) => id,
        state => return Err(IntentPhonemeRefusal::Unresolved(state)),
    };
    let mut matches = inventory
        .phonemes()
        .as_slice()
        .iter()
        .filter(|definition| definition.identity() == requested);
    let definition = matches
        .next()
        .ok_or(IntentPhonemeRefusal::MissingDefinition)?;
    if matches.next().is_some() {
        return Err(IntentPhonemeRefusal::AmbiguousDefinition);
    }
    let identity =
        SpeechPhonemeDefinitionMatch::new(definition.identity().clone(), requested.clone())
            .map_err(IntentPhonemeRefusal::Definition)?;
    Ok(ResolvedIntentPhoneme {
        occurrence,
        inventory,
        definition,
        basis,
        identity,
    })
}
