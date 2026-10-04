//! Bounded preparation traversal over exact intent occurrences.
//! Native Conduit laws own membership; this is neither full speech admission
//! nor artifact resolution, commitment, or a pronunciation decision.
use crate::semantic::{
    SpeechOccurrenceMembership, SpeechUtteranceIntent, SpeechUtteranceIntentEvent,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub struct OccurrenceMembershipRefusal {
    pub event: usize,
    pub reason: NativeBindingRefusal,
}

/// Check segment occurrences against the intent's declared basis. Source
/// references can legitimately name other revisions and remain untouched.
/// Boundaries have sources but no synthesized token occurrence to bind.
/// All six specification states remain unresolved. Allocation, when required
/// by native checking, occurs only during preparation of this bounded sequence.
pub fn validate_intent_occurrences(
    intent: &SpeechUtteranceIntent,
) -> Result<(), OccurrenceMembershipRefusal> {
    for (event, value) in intent.events().as_slice().iter().enumerate() {
        if let SpeechUtteranceIntentEvent::Segment(segment) = value {
            SpeechOccurrenceMembership::new(
                intent.inventory_id().clone(),
                intent.language().clone(),
                segment.occurrence().clone(),
                intent.revision_id().clone(),
                intent.utterance_id().clone(),
            )
            .map_err(|reason| OccurrenceMembershipRefusal { event, reason })?;
        }
    }
    Ok(())
}
