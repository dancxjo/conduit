//! Immediate planned event context with exact occurrence membership and adjacency.
//! An unobserved endpoint is unknown: the intent carries no sequence closure.
use crate::{
    intent_admission::{validate_intent_occurrences, OccurrenceMembershipRefusal},
    neighbor_match::NeighborObservation,
    rule_conditions::ConditionEvidence,
    rule_stress::StressObservation,
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum OccurrenceContextRefusal {
    MissingEvent,
    BoundaryEvent,
    Membership(OccurrenceMembershipRefusal),
    DuplicateOccurrence {
        first: usize,
        second: usize,
    },
    Neighbor {
        event: usize,
        reason: NativeBindingRefusal,
    },
}

// Bounded preparation receipts stay inline instead of adding boxed allocations.
#[allow(clippy::large_enum_variant)]
pub enum PlannedNeighbor<'a> {
    Unobserved,
    Boundary {
        event: usize,
        boundary: &'a SpeechUtteranceIntentEventBoundary,
    },
    Segment {
        event: usize,
        segment: &'a SpeechUtteranceIntentEventSegment,
        membership: SpeechOccurrenceMembership,
        adjacency: SpeechOccurrenceAdjacency,
    },
}
impl<'a> PlannedNeighbor<'a> {
    pub fn observation(&self) -> NeighborObservation<'a> {
        match self {
            Self::Unobserved => NeighborObservation::Unknown,
            Self::Boundary { boundary, .. } => NeighborObservation::Boundary(boundary.kind()),
            Self::Segment { segment, .. } => NeighborObservation::Segment {
                phone: segment.phone(),
                phoneme: segment.phoneme(),
                features: None,
            },
        }
    }
    pub fn stress(&self) -> StressObservation<'a> {
        match self {
            Self::Unobserved => StressObservation::Unknown,
            Self::Boundary { boundary, .. } => StressObservation::Boundary(boundary.kind()),
            Self::Segment { segment, .. } => StressObservation::Segment(segment.stress()),
        }
    }
}

pub struct IntentOccurrenceContext<'a> {
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    segment: &'a SpeechUtteranceIntentEventSegment,
    membership: SpeechOccurrenceMembership,
    before: PlannedNeighbor<'a>,
    after: PlannedNeighbor<'a>,
}
impl<'a> IntentOccurrenceContext<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn event(&self) -> usize {
        self.event
    }
    pub fn segment(&self) -> &'a SpeechUtteranceIntentEventSegment {
        self.segment
    }
    pub fn membership(&self) -> &SpeechOccurrenceMembership {
        &self.membership
    }
    pub fn before(&self) -> &PlannedNeighbor<'a> {
        &self.before
    }
    pub fn after(&self) -> &PlannedNeighbor<'a> {
        &self.after
    }
    /// Both matcher and stress views come from the same retained event neighbors.
    /// Feature evidence remains unknown; definitions do not become observations.
    pub fn condition_evidence(
        &self,
        careful_style: &'a SpeechCarefulStyleSpecification,
    ) -> ConditionEvidence<'a> {
        ConditionEvidence {
            before: self.before.observation(),
            after: self.after.observation(),
            before_stress: self.before.stress(),
            after_stress: self.after.stress(),
            careful_style,
        }
    }
}
fn membership(
    intent: &SpeechUtteranceIntent,
    segment: &SpeechUtteranceIntentEventSegment,
) -> Result<SpeechOccurrenceMembership, NativeBindingRefusal> {
    SpeechOccurrenceMembership::new(
        intent.inventory_id().clone(),
        intent.language().clone(),
        segment.occurrence().clone(),
        intent.revision_id().clone(),
        intent.utterance_id().clone(),
    )
}
pub fn resolve_intent_occurrence_context(
    intent: &SpeechUtteranceIntent,
    event: usize,
) -> Result<IntentOccurrenceContext<'_>, OccurrenceContextRefusal> {
    let segment = match intent.events().as_slice().get(event) {
        Some(SpeechUtteranceIntentEvent::Segment(value)) => value,
        Some(SpeechUtteranceIntentEvent::Boundary(_)) => {
            return Err(OccurrenceContextRefusal::BoundaryEvent)
        }
        None => return Err(OccurrenceContextRefusal::MissingEvent),
    };
    validate_intent_occurrences(intent).map_err(OccurrenceContextRefusal::Membership)?;
    // At most 256 events: reject duplicate scoped occurrence identities before
    // producing contextual evidence. No ordinal is reinterpreted as an index.
    for (second, candidate) in intent.events().as_slice().iter().enumerate() {
        let SpeechUtteranceIntentEvent::Segment(candidate) = candidate else {
            continue;
        };
        for (first, previous) in intent.events().as_slice()[..second].iter().enumerate() {
            if matches!(previous, SpeechUtteranceIntentEvent::Segment(previous) if previous.occurrence() == candidate.occurrence())
            {
                return Err(OccurrenceContextRefusal::DuplicateOccurrence { first, second });
            }
        }
    }
    let checked = membership(intent, segment)
        .map_err(|reason| OccurrenceContextRefusal::Neighbor { event, reason })?;
    let neighbor = |index: Option<usize>,
                    before: bool|
     -> Result<PlannedNeighbor<'_>, OccurrenceContextRefusal> {
        let Some((index, value)) = index.and_then(|index| {
            intent
                .events()
                .as_slice()
                .get(index)
                .map(|value| (index, value))
        }) else {
            return Ok(PlannedNeighbor::Unobserved);
        };
        Ok(match value {
            SpeechUtteranceIntentEvent::Boundary(boundary) => PlannedNeighbor::Boundary {
                event: index,
                boundary,
            },
            SpeechUtteranceIntentEvent::Segment(other) => {
                let (earlier, later) = if before {
                    (other.occurrence(), segment.occurrence())
                } else {
                    (segment.occurrence(), other.occurrence())
                };
                let adjacency = SpeechOccurrenceAdjacency::new(earlier.clone(), later.clone())
                    .map_err(|reason| OccurrenceContextRefusal::Neighbor {
                        event: index,
                        reason,
                    })?;
                let membership = membership(intent, other).map_err(|reason| {
                    OccurrenceContextRefusal::Neighbor {
                        event: index,
                        reason,
                    }
                })?;
                PlannedNeighbor::Segment {
                    event: index,
                    segment: other,
                    membership,
                    adjacency,
                }
            }
        })
    };
    Ok(IntentOccurrenceContext {
        intent,
        event,
        segment,
        membership: checked,
        before: neighbor(event.checked_sub(1), true)?,
        after: neighbor(Some(event + 1), false)?,
    })
}
