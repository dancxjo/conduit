//! Scalar evidence for an original allophone environment. Neighbor patterns,
//! conditions and rule status remain obligations; this receipt is not eligibility.
use crate::{context_match::*, semantic::*};

/// Explicit observations for one occurrence. No missing value is inferred.
pub struct ScalarContextObservation<'a> {
    pub stress: &'a StressSpecification,
    pub word_position: &'a SpeechPositionSpecification,
    pub syllable_position: &'a SpeechSyllablePositionSpecification,
    pub prosodic_context: &'a SpeechProsodicContextSpecification,
}

pub struct AllophoneScalarContext<'a> {
    declaration: &'a SpeechPhonemeAllophone,
    stress: ContextComparison<'a, StressSpecification>,
    word_position: ContextComparison<'a, SpeechPositionSpecification>,
    syllable_position: ContextComparison<'a, SpeechSyllablePositionSpecification>,
    prosodic_context: ContextComparison<'a, SpeechProsodicContextSpecification>,
}
impl<'a> AllophoneScalarContext<'a> {
    pub fn stress(&self) -> &ContextComparison<'a, StressSpecification> {
        &self.stress
    }

    pub fn word_position(&self) -> &ContextComparison<'a, SpeechPositionSpecification> {
        &self.word_position
    }

    pub fn syllable_position(&self) -> &ContextComparison<'a, SpeechSyllablePositionSpecification> {
        &self.syllable_position
    }

    pub fn prosodic_context(&self) -> &ContextComparison<'a, SpeechProsodicContextSpecification> {
        &self.prosodic_context
    }

    /// Retains neighbor alternatives, conditions, confidence and rule status.
    pub fn declaration(&self) -> &'a SpeechPhonemeAllophone {
        self.declaration
    }
}

pub fn compare_allophone_scalar_context<'a>(
    declaration: &'a SpeechPhonemeAllophone,
    observation: ScalarContextObservation<'a>,
) -> Result<AllophoneScalarContext<'a>, ContextComparisonRefusal> {
    let environment = declaration.environment();
    Ok(AllophoneScalarContext {
        declaration,
        stress: compare_stress(environment.stress_context(), observation.stress)?,
        word_position: compare_word_position(
            environment.word_position(),
            observation.word_position,
        )?,
        syllable_position: compare_syllable_position(
            environment.syllable_position(),
            observation.syllable_position,
        )?,
        prosodic_context: compare_prosodic_context(
            environment.prosodic_context(),
            observation.prosodic_context,
        )?,
    })
}

/// Original before/after requirements compared against explicitly supplied
/// immediate neighbors. Scalar and condition obligations remain separate.
pub struct AllophoneNeighborContext<'a> {
    declaration: &'a SpeechPhonemeAllophone,
    before: crate::neighbor_match::NeighborAlternatives<'a>,
    after: crate::neighbor_match::NeighborAlternatives<'a>,
}
impl<'a> AllophoneNeighborContext<'a> {
    pub fn declaration(&self) -> &'a SpeechPhonemeAllophone {
        self.declaration
    }
    pub fn before(&self) -> &crate::neighbor_match::NeighborAlternatives<'a> {
        &self.before
    }
    pub fn after(&self) -> &crate::neighbor_match::NeighborAlternatives<'a> {
        &self.after
    }
}
pub fn compare_allophone_neighbors<'a>(
    declaration: &'a SpeechPhonemeAllophone,
    before: crate::neighbor_match::NeighborObservation<'a>,
    after: crate::neighbor_match::NeighborObservation<'a>,
) -> Result<AllophoneNeighborContext<'a>, crate::neighbor_match::NeighborComparisonRefusal> {
    use crate::neighbor_match::compare_neighbor_alternatives;
    Ok(AllophoneNeighborContext {
        declaration,
        before: compare_neighbor_alternatives(
            declaration.environment().before().as_slice(),
            before,
        )?,
        after: compare_neighbor_alternatives(declaration.environment().after().as_slice(), after)?,
    })
}
