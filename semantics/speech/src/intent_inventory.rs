//! Exact inventory lookup for a requested segment, without material evidence.
use crate::semantic::*;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum IntentInventoryRefusal {
    MissingEvent,
    BoundaryEvent,
    Occurrence(NativeBindingRefusal),
    Basis(NativeBindingRefusal),
    Unresolved(PhoneSpecification),
    MissingDefinition,
    AmbiguousDefinition,
    Definition(NativeBindingRefusal),
}

/// Preparation receipt retaining the actual intent event and full definition.
/// Lookup does not establish authenticity, authority, or a selected voice.
pub struct ResolvedIntentPhone<'a> {
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    segment: &'a SpeechUtteranceIntentEventSegment,
    inventory: &'a SpeechInventory,
    definition: &'a SpeechPhone,
    occurrence: SpeechOccurrenceMembership,
    basis: SpeechIntentInventoryBasis,
    identity: SpeechPhoneDefinitionMatch,
}
impl<'a> ResolvedIntentPhone<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn event(&self) -> usize {
        self.event
    }
    pub fn segment(&self) -> &'a SpeechUtteranceIntentEventSegment {
        self.segment
    }
    pub fn inventory(&self) -> &'a SpeechInventory {
        self.inventory
    }
    pub fn definition(&self) -> &'a SpeechPhone {
        self.definition
    }
    pub fn checked_occurrence(&self) -> &SpeechOccurrenceMembership {
        &self.occurrence
    }
    pub fn checked_basis(&self) -> &SpeechIntentInventoryBasis {
        &self.basis
    }
    pub fn checked_identity(&self) -> &SpeechPhoneDefinitionMatch {
        &self.identity
    }
}

pub fn resolve_intent_inventory_phone<'a>(
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    inventory: &'a SpeechInventory,
) -> Result<ResolvedIntentPhone<'a>, IntentInventoryRefusal> {
    let segment = match intent.events().as_slice().get(event) {
        Some(SpeechUtteranceIntentEvent::Segment(segment)) => segment,
        Some(SpeechUtteranceIntentEvent::Boundary(_)) => {
            return Err(IntentInventoryRefusal::BoundaryEvent)
        }
        None => return Err(IntentInventoryRefusal::MissingEvent),
    };
    let occurrence = SpeechOccurrenceMembership::new(
        intent.inventory_id().clone(),
        intent.language().clone(),
        segment.occurrence().clone(),
        intent.revision_id().clone(),
        intent.utterance_id().clone(),
    )
    .map_err(IntentInventoryRefusal::Occurrence)?;
    let basis = SpeechIntentInventoryBasis::new(
        intent.inventory_id().clone(),
        intent.language().clone(),
        inventory.identity().clone(),
        inventory.language().clone(),
    )
    .map_err(IntentInventoryRefusal::Basis)?;
    let requested = match segment.phone() {
        PhoneSpecification::Known(id) => id,
        state => return Err(IntentInventoryRefusal::Unresolved(state.clone())),
    };
    let mut matches = inventory
        .phones()
        .as_slice()
        .iter()
        .filter(|definition| definition.identity() == requested);
    let definition = matches
        .next()
        .ok_or(IntentInventoryRefusal::MissingDefinition)?;
    if matches.next().is_some() {
        return Err(IntentInventoryRefusal::AmbiguousDefinition);
    }
    let identity =
        SpeechPhoneDefinitionMatch::new(definition.identity().clone(), requested.clone())
            .map_err(IntentInventoryRefusal::Definition)?;
    Ok(ResolvedIntentPhone {
        intent,
        event,
        segment,
        inventory,
        definition,
        occurrence,
        basis,
        identity,
    })
}
