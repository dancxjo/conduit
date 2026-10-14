//! A legacy rule condition consumes one exact language-owned dependency arc.
//! Supplied arcs are evidence, not claims about parser accuracy or punctuation.
use crate::semantic::{SpeechContextDecision, SpeechRuleCondition};
use conduit_language::{
    lexical::PreparedLexicalTape, LanguageAnalysisRevisionId, LanguageDependencyArc,
    LanguageDependencyHead, LanguageUniversalDependencyRelation, LanguageVariety,
    LinguisticSyntacticLinkKind,
};

#[derive(Clone, Copy)]
pub struct SyntaxObservation<'a> {
    pub lexical: &'a PreparedLexicalTape,
    pub analysis: &'a LanguageAnalysisRevisionId,
    pub arc: &'a LanguageDependencyArc,
    pub ordinal: usize,
    pub expected_variety: &'a LanguageVariety,
    pub variety: &'a LanguageVariety,
}
#[derive(Debug)]
pub enum SyntaxConditionRefusal {
    UnsupportedCondition,
    UnsupportedLegacyKind,
    UnsupportedSubtype,
    Variety,
    Token,
    ForeignArc,
}
pub struct SyntaxConditionComparison<'a> {
    condition: &'a SpeechRuleCondition,
    observation: SyntaxObservation<'a>,
    decision: SpeechContextDecision,
}
impl<'a> SyntaxConditionComparison<'a> {
    pub fn condition(&self) -> &'a SpeechRuleCondition {
        self.condition
    }
    pub fn observation(&self) -> &SyntaxObservation<'a> {
        &self.observation
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}

pub fn compare_syntax_condition<'a>(
    condition: &'a SpeechRuleCondition,
    observation: SyntaxObservation<'a>,
) -> Result<SyntaxConditionComparison<'a>, SyntaxConditionRefusal> {
    use SyntaxConditionRefusal as R;
    let SpeechRuleCondition::CurrentWordHasSyntacticLink(legacy) = condition else {
        return Err(R::UnsupportedCondition);
    };
    // This bounded migration supports the unambiguous vocative category only.
    // Other legacy categories cannot silently collapse UD distinctions.
    if !matches!(legacy, LinguisticSyntacticLinkKind::Vocative) {
        return Err(R::UnsupportedLegacyKind);
    }
    let tape = observation.lexical.tape();
    if observation.variety != observation.expected_variety
        || observation.variety.language() != tape.source().material().language()
    {
        return Err(R::Variety);
    }
    let token = tape
        .tokens()
        .as_slice()
        .get(observation.ordinal)
        .ok_or(R::Token)?;
    if !matches!(
        token.category(),
        conduit_language::LinguisticTokenCategory::Word
    ) || !matches!(
        token.completeness(),
        conduit_language::LanguageLexicalCompleteness::Complete
    ) {
        return Err(R::Token);
    }
    let valid = |identity: &conduit_language::LinguisticTokenIdentity| {
        tape.tokens()
            .iter()
            .any(|token| token.identity() == identity)
    };
    if observation.arc.dependent().revision() != observation.analysis
        || observation.arc.dependent().token() != token.identity()
        || !valid(observation.arc.dependent().token())
        || !matches!(observation.arc.governor(), LanguageDependencyHead::Token(head)
            if head.revision() == observation.analysis && valid(head.token()))
    {
        return Err(R::ForeignArc);
    }
    if observation.arc.relation().subtype().is_some() {
        return Err(R::UnsupportedSubtype);
    }
    let decision = if matches!(
        observation.arc.relation().base(),
        LanguageUniversalDependencyRelation::Vocative
    ) {
        SpeechContextDecision::Matched
    } else {
        SpeechContextDecision::Mismatched
    };
    Ok(SyntaxConditionComparison {
        condition,
        observation,
        decision,
    })
}
