//! Input and contextual obligations from one exact original occurrence.
//! Evaluation is not rule permission, selection, inheritance or commitment.
use crate::{
    declared_context::ExplicitAllophoneContext,
    generated,
    rule_context::{compare_allophone_rule_context, AllophoneRuleContext, RuleContextRefusal},
    rule_input::{compare_allophone_rule_input, AllophoneRuleInputComparison, RuleInputRefusal},
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum RuleEvaluationRefusal {
    Context(RuleContextRefusal),
    FeatureOccurrence(NativeBindingRefusal),
    Input(RuleInputRefusal),
    CompiledPlot,
}
pub struct AllophoneRuleEvaluation<'a> {
    context: AllophoneRuleContext<'a>,
    input: AllophoneRuleInputComparison<'a>,
    observed_features: Option<&'a SpeechOccurrenceFeatureObservation>,
    checked_features: Option<SpeechOccurrenceObservationMatch>,
    decision: SpeechContextDecision,
}
impl<'a> AllophoneRuleEvaluation<'a> {
    pub fn rule(&self) -> &'a SpeechAllophoneRule {
        self.context.rule()
    }
    pub fn context(&self) -> &AllophoneRuleContext<'a> {
        &self.context
    }
    pub fn input(&self) -> &AllophoneRuleInputComparison<'a> {
        &self.input
    }
    pub fn observed_features(&self) -> Option<&'a SpeechOccurrenceFeatureObservation> {
        self.observed_features
    }
    pub fn checked_features(&self) -> Option<&SpeechOccurrenceObservationMatch> {
        self.checked_features.as_ref()
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
fn projected(value: &SpeechContextDecision) -> generated::SpeechContextDecision {
    match value {
        SpeechContextDecision::Matched => generated::SpeechContextDecision::matched,
        SpeechContextDecision::Mismatched => generated::SpeechContextDecision::mismatched,
        SpeechContextDecision::RequirementUnresolved => {
            generated::SpeechContextDecision::requirement_unresolved
        }
        SpeechContextDecision::ObservationUnresolved => {
            generated::SpeechContextDecision::observation_unresolved
        }
    }
}
/// Source phoneme, scalar and neighbor evidence come from this same intent.
/// Optional current features must name this exact occurrence; their provenance
/// is retained without claiming source resolution, inheritance or authentication.
pub fn evaluate_allophone_rule<'a>(
    rule: &'a SpeechAllophoneRule,
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    observed_features: Option<&'a SpeechOccurrenceFeatureObservation>,
    explicit: ExplicitAllophoneContext<'a>,
) -> Result<AllophoneRuleEvaluation<'a>, RuleEvaluationRefusal> {
    let context = compare_allophone_rule_context(rule, intent, event, explicit)
        .map_err(RuleEvaluationRefusal::Context)?;
    let checked_features = observed_features
        .map(|observation| {
            SpeechOccurrenceObservationMatch::new(
                context.occurrence().segment().occurrence().clone(),
                observation.occurrence().clone(),
            )
        })
        .transpose()
        .map_err(RuleEvaluationRefusal::FeatureOccurrence)?;
    let input = compare_allophone_rule_input(
        rule,
        context.occurrence().segment().phoneme(),
        observed_features.map(|observation| observation.features()),
    )
    .map_err(RuleEvaluationRefusal::Input)?;
    let result = generated::speech_context_conjunction(generated::SpeechContextConjunction {
        left: projected(input.decision()),
        right: projected(context.decision()),
    })
    .ok_or(RuleEvaluationRefusal::CompiledPlot)?;
    let decision = match result {
        generated::SpeechContextDecision::matched => SpeechContextDecision::Matched,
        generated::SpeechContextDecision::mismatched => SpeechContextDecision::Mismatched,
        generated::SpeechContextDecision::requirement_unresolved => {
            SpeechContextDecision::RequirementUnresolved
        }
        generated::SpeechContextDecision::observation_unresolved => {
            SpeechContextDecision::ObservationUnresolved
        }
    };
    Ok(AllophoneRuleEvaluation {
        context,
        input,
        observed_features,
        checked_features,
        decision,
    })
}
