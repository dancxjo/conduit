//! Immediate-neighbor comparisons. Pattern lists are alternatives, never a
//! sequence of offsets. Identity equality is checked by native Conduit laws.
use crate::semantic::*;
use conduit_plot::rust_binding::NativeBindingRefusal;

/// Absence and missing observation remain different, including in receipts.
pub enum NeighborObservation<'a> {
    Absent,
    Unknown,
    Segment {
        phone: &'a PhoneSpecification,
        phoneme: &'a PhonemeSpecification,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub enum NeighborDecision {
    Matched,
    Mismatched,
    ObservationUnresolved,
    UnsupportedMatcher,
}
pub struct NeighborComparison<'a> {
    requirement: &'a SpeechSegmentMatcher,
    observation: NeighborObservation<'a>,
    decision: NeighborDecision,
}
impl<'a> NeighborComparison<'a> {
    pub fn requirement(&self) -> &'a SpeechSegmentMatcher {
        self.requirement
    }
    pub fn observation(&self) -> &NeighborObservation<'a> {
        &self.observation
    }
    pub fn decision(&self) -> &NeighborDecision {
        &self.decision
    }
}

fn identity_decision<T>(
    checked: Result<T, NativeBindingRefusal>,
) -> Result<NeighborDecision, NativeBindingRefusal> {
    match checked {
        Ok(_) => Ok(NeighborDecision::Matched),
        Err(NativeBindingRefusal::ViolatedInvariant { index: 0 }) => {
            Ok(NeighborDecision::Mismatched)
        }
        Err(reason) => Err(reason),
    }
}

pub fn compare_neighbor<'a>(
    requirement: &'a SpeechSegmentMatcher,
    observation: NeighborObservation<'a>,
) -> Result<NeighborComparison<'a>, NativeBindingRefusal> {
    let decision = match requirement {
        SpeechSegmentMatcher::Features(_) | SpeechSegmentMatcher::Boundary(_) => {
            NeighborDecision::UnsupportedMatcher
        }
        _ => match &observation {
            NeighborObservation::Absent => NeighborDecision::Mismatched,
            NeighborObservation::Unknown => NeighborDecision::ObservationUnresolved,
            NeighborObservation::Segment { phone, phoneme } => match requirement {
                SpeechSegmentMatcher::Any => NeighborDecision::Matched,
                SpeechSegmentMatcher::Phone(expected) => match phone {
                    PhoneSpecification::Known(actual) => identity_decision(
                        SpeechPhoneDefinitionMatch::new(actual.clone(), expected.clone()),
                    )?,
                    _ => NeighborDecision::ObservationUnresolved,
                },
                SpeechSegmentMatcher::Phoneme(expected) => match phoneme {
                    PhonemeSpecification::Known(actual) => identity_decision(
                        SpeechPhonemeDefinitionMatch::new(actual.clone(), expected.clone()),
                    )?,
                    _ => NeighborDecision::ObservationUnresolved,
                },
                _ => unreachable!(),
            },
        },
    };
    Ok(NeighborComparison {
        requirement,
        observation,
        decision,
    })
}
