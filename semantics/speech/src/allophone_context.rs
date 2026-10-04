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
    pub stress: ContextComparison<'a, StressSpecification>,
    pub word_position: ContextComparison<'a, SpeechPositionSpecification>,
    pub syllable_position: ContextComparison<'a, SpeechSyllablePositionSpecification>,
    pub prosodic_context: ContextComparison<'a, SpeechProsodicContextSpecification>,
}
impl<'a> AllophoneScalarContext<'a> {
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
