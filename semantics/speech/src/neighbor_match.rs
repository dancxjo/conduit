//! Immediate-neighbor comparisons. Pattern lists are alternatives, never a
//! sequence of offsets. Identity equality is checked by native Conduit laws.
pub use crate::semantic::SpeechNeighborDecision as NeighborDecision;
use crate::{
    feature_bundle::{
        compare_feature_bundle_observation, FeatureBundleComparison, FeatureBundleRefusal,
    },
    generated,
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

/// Absence and missing observation remain different, including in receipts.
#[derive(Clone, Copy)]
pub enum NeighborObservation<'a> {
    Absent,
    Unknown,
    Boundary(&'a SpeechBoundarySpecification),
    Segment {
        phone: &'a PhoneSpecification,
        phoneme: &'a PhonemeSpecification,
        features: Option<&'a SpeechFeatureBundle>,
    },
}
#[derive(Debug)]
pub enum NeighborComparisonRefusal {
    Native(NativeBindingRefusal),
    CompiledPlot,
    Features(FeatureBundleRefusal),
    TooManyAlternatives { actual: usize },
}
pub struct NeighborComparison<'a> {
    requirement: &'a SpeechSegmentMatcher,
    observation: NeighborObservation<'a>,
    decision: NeighborDecision,
    features: Option<FeatureBundleComparison<'a>>,
}
impl<'a> NeighborComparison<'a> {
    pub fn features(&self) -> Option<&FeatureBundleComparison<'a>> {
        self.features.as_ref()
    }
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

fn identity_fact<T>(
    checked: Result<T, NativeBindingRefusal>,
    invariant_count: usize,
) -> Result<generated::SpeechNeighborIdentity, NeighborComparisonRefusal> {
    use generated::SpeechNeighborIdentity as I;
    match checked {
        Ok(_) => Ok(I::matched),
        Err(NativeBindingRefusal::ViolatedInvariant { index }) if index < invariant_count => {
            Ok(I::mismatched)
        }
        Err(reason) => Err(NeighborComparisonRefusal::Native(reason)),
    }
}

pub fn compare_neighbor<'a>(
    requirement: &'a SpeechSegmentMatcher,
    observation: NeighborObservation<'a>,
) -> Result<NeighborComparison<'a>, NeighborComparisonRefusal> {
    use generated::{
        SpeechNeighborIdentity as I, SpeechNeighborMatcherKind as M, SpeechNeighborPresence as P,
    };
    let matcher = match requirement {
        SpeechSegmentMatcher::Any => M::any,
        SpeechSegmentMatcher::Phone(_) | SpeechSegmentMatcher::Phoneme(_) => M::segment_identity,
        SpeechSegmentMatcher::Boundary(_) => M::boundary_identity,
        SpeechSegmentMatcher::Features(_) => M::segment_identity,
    };
    let presence = match &observation {
        NeighborObservation::Absent => P::absent,
        NeighborObservation::Unknown => P::unknown,
        NeighborObservation::Segment { .. } => P::segment,
        NeighborObservation::Boundary(_) => P::boundary,
    };
    // Only exact Known identities cross the native equality law. Other states
    // retain their original borrowed evidence and project an unresolved fact.
    let features = match (requirement, &observation) {
        (
            SpeechSegmentMatcher::Features(expected),
            NeighborObservation::Segment { features, .. },
        ) => Some(
            compare_feature_bundle_observation(expected, *features)
                .map_err(NeighborComparisonRefusal::Features)?,
        ),
        _ => None,
    };
    let identity = match (requirement, &observation) {
        (
            SpeechSegmentMatcher::Phone(expected),
            NeighborObservation::Segment {
                phone: PhoneSpecification::Known(actual),
                ..
            },
        ) => identity_fact(
            SpeechPhoneDefinitionMatch::new(actual.clone(), expected.clone()),
            1,
        )?,
        (
            SpeechSegmentMatcher::Phoneme(expected),
            NeighborObservation::Segment {
                phoneme: PhonemeSpecification::Known(actual),
                ..
            },
        ) => identity_fact(
            SpeechPhonemeDefinitionMatch::new(actual.clone(), expected.clone()),
            1,
        )?,
        (
            SpeechSegmentMatcher::Boundary(expected),
            NeighborObservation::Boundary(SpeechBoundarySpecification::Known(actual)),
        ) => identity_fact(SpeechBoundaryIntentMatch::new(*expected, *actual), 7)?,
        (SpeechSegmentMatcher::Features(_), NeighborObservation::Segment { .. }) => match features
            .as_ref()
            .expect("feature comparison retained")
            .decision()
        {
            SpeechContextDecision::Matched => I::matched,
            SpeechContextDecision::Mismatched => I::mismatched,
            SpeechContextDecision::RequirementUnresolved => I::requirement_unresolved,
            SpeechContextDecision::ObservationUnresolved => I::unresolved,
        },
        _ => I::unresolved,
    };
    let result = generated::speech_neighbor_compare(generated::SpeechNeighborComparisonInput {
        matcher,
        presence,
        identity,
    })
    .ok_or(NeighborComparisonRefusal::CompiledPlot)?;
    let decision = decision_from_generated(result);
    Ok(NeighborComparison {
        requirement,
        observation,
        decision,
        features,
    })
}

fn decision_from_generated(result: generated::SpeechNeighborDecision) -> NeighborDecision {
    match result {
        generated::SpeechNeighborDecision::matched => NeighborDecision::Matched,
        generated::SpeechNeighborDecision::mismatched => NeighborDecision::Mismatched,
        generated::SpeechNeighborDecision::observation_unresolved => {
            NeighborDecision::ObservationUnresolved
        }
        generated::SpeechNeighborDecision::requirement_unresolved => {
            NeighborDecision::RequirementUnresolved
        }
        generated::SpeechNeighborDecision::unsupported_matcher => {
            NeighborDecision::UnsupportedMatcher
        }
    }
}

/// All alternatives concern this same immediate neighbor, with at most four
/// fixed receipt slots. Empty lists are unconstrained even at an observed edge.
pub struct NeighborAlternatives<'a> {
    receipts: [Option<NeighborComparison<'a>>; 4],
    count: usize,
    decision: NeighborDecision,
}
impl<'a> NeighborAlternatives<'a> {
    pub fn comparisons(&self) -> impl Iterator<Item = &NeighborComparison<'a>> {
        self.receipts[..self.count]
            .iter()
            .filter_map(Option::as_ref)
    }
    pub fn decision(&self) -> &NeighborDecision {
        &self.decision
    }
}
pub fn compare_neighbor_alternatives<'a>(
    requirements: &'a [SpeechSegmentMatcher],
    observation: NeighborObservation<'a>,
) -> Result<NeighborAlternatives<'a>, NeighborComparisonRefusal> {
    if requirements.len() > 4 {
        return Err(NeighborComparisonRefusal::TooManyAlternatives {
            actual: requirements.len(),
        });
    }
    let mut receipts = core::array::from_fn(|_| None);
    let mut accumulated = generated::SpeechNeighborDecision::mismatched;
    for (index, requirement) in requirements.iter().enumerate() {
        let comparison = compare_neighbor(requirement, observation)?;
        let right = match comparison.decision() {
            NeighborDecision::Matched => generated::SpeechNeighborDecision::matched,
            NeighborDecision::Mismatched => generated::SpeechNeighborDecision::mismatched,
            NeighborDecision::ObservationUnresolved => {
                generated::SpeechNeighborDecision::observation_unresolved
            }
            NeighborDecision::RequirementUnresolved => {
                generated::SpeechNeighborDecision::requirement_unresolved
            }
            NeighborDecision::UnsupportedMatcher => {
                generated::SpeechNeighborDecision::unsupported_matcher
            }
        };
        accumulated =
            generated::speech_neighbor_alternative(generated::SpeechNeighborAlternativePair {
                left: accumulated,
                right,
            })
            .ok_or(NeighborComparisonRefusal::CompiledPlot)?;
        receipts[index] = Some(comparison);
    }
    let result = generated::speech_neighbor_alternatives_finish(
        generated::SpeechNeighborAlternativeFinish {
            empty: requirements.is_empty(),
            accumulated,
        },
    )
    .ok_or(NeighborComparisonRefusal::CompiledPlot)?;
    Ok(NeighborAlternatives {
        receipts,
        count: requirements.len(),
        decision: decision_from_generated(result),
    })
}
