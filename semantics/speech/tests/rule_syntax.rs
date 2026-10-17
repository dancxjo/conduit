#![cfg(feature = "semantic-bindings")]
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{rule_syntax::*, semantic::*};
fn lexical(revision: &str) -> PreparedLexicalTape {
    let language = LanguageId::new("language/en".into()).unwrap();
    let provenance =
        LinguisticDerivationProvenance::deterministic_rule("syntax-fixture".into(), "1".into())
            .unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("source/1".into()).unwrap(),
            language.clone(),
            LanguageTextRevisionId::new(revision.into()).unwrap(),
            "hello Travis".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        None,
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::new(),
        "lexical/1".into(),
        language,
        provenance,
    )
    .unwrap();
    prepare_lexical_tape(&source, &profile, None).unwrap()
}
fn analysis() -> LanguageAnalysisRevisionId {
    LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap()
}
fn variety(id: &str) -> LanguageVariety {
    LanguageVariety::new(
        VarietyId::new(id.into()).unwrap(),
        LanguageId::new("language/en".into()).unwrap(),
    )
    .unwrap()
}
fn arc(
    lexical: &PreparedLexicalTape,
    base: LanguageUniversalDependencyRelation,
    subtype: Option<LanguageDependencySubtype>,
) -> LanguageDependencyArc {
    LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(analysis(), lexical.tape().tokens()[1].identity().clone())
            .unwrap(),
        LanguageDependencyHead::token(analysis(), lexical.tape().tokens()[0].identity().clone())
            .unwrap(),
        LanguageDependencyRelation::new(base, subtype).unwrap(),
    )
    .unwrap()
}
#[test]
fn existing_vocative_condition_consumes_exact_language_arc_without_punctuation() {
    let lexical = lexical("r1");
    let variety = variety("variety/local");
    let analysis = analysis();
    let original = arc(
        &lexical,
        LanguageUniversalDependencyRelation::Vocative,
        None,
    );
    let transported = LanguageDependencyArc::decode(&original.clone().encode().unwrap()).unwrap();
    let condition =
        SpeechRuleCondition::current_word_has_syntactic_link(LinguisticSyntacticLinkKind::Vocative)
            .unwrap();
    let result = compare_syntax_condition(
        &condition,
        SyntaxObservation {
            lexical: &lexical,
            analysis: &analysis,
            arc: &transported,
            ordinal: 1,
            expected_variety: &variety,
            variety: &variety,
        },
    )
    .unwrap();
    assert_eq!(result.decision(), &SpeechContextDecision::Matched);
    assert!(core::ptr::eq(result.observation().lexical, &lexical));
    assert_eq!(result.observation().arc, &original);
    assert_eq!(result.condition(), &condition);
    assert_eq!(result.observation().variety, &variety);
    let nonvocative = arc(&lexical, LanguageUniversalDependencyRelation::Nsubj, None);
    assert_eq!(
        compare_syntax_condition(
            &condition,
            SyntaxObservation {
                arc: &nonvocative,
                ..*result.observation()
            }
        )
        .unwrap()
        .decision(),
        &SpeechContextDecision::Mismatched
    );
}
#[test]
fn unknown_legacy_categories_subtypes_and_foreign_revisions_refuse() {
    let lexical = lexical("r1");
    let foreign = self::lexical("r2");
    let variety = variety("variety/local");
    let other = self::variety("variety/other");
    let analysis = analysis();
    let original = arc(
        &lexical,
        LanguageUniversalDependencyRelation::Vocative,
        None,
    );
    let condition =
        SpeechRuleCondition::current_word_has_syntactic_link(LinguisticSyntacticLinkKind::Vocative)
            .unwrap();
    let observation = SyntaxObservation {
        lexical: &lexical,
        analysis: &analysis,
        arc: &original,
        ordinal: 1,
        expected_variety: &variety,
        variety: &variety,
    };
    let unsupported = SpeechRuleCondition::current_word_has_syntactic_link(
        LinguisticSyntacticLinkKind::Parenthetical,
    )
    .unwrap();
    assert!(matches!(
        compare_syntax_condition(&unsupported, observation),
        Err(SyntaxConditionRefusal::UnsupportedLegacyKind)
    ));
    let subtype = arc(
        &lexical,
        LanguageUniversalDependencyRelation::Vocative,
        Some(LanguageDependencySubtype::new("unreviewed".into()).unwrap()),
    );
    assert!(matches!(
        compare_syntax_condition(
            &condition,
            SyntaxObservation {
                arc: &subtype,
                ..observation
            }
        ),
        Err(SyntaxConditionRefusal::UnsupportedSubtype)
    ));
    let stale = arc(
        &foreign,
        LanguageUniversalDependencyRelation::Vocative,
        None,
    );
    assert!(matches!(
        compare_syntax_condition(
            &condition,
            SyntaxObservation {
                arc: &stale,
                ..observation
            }
        ),
        Err(SyntaxConditionRefusal::ForeignArc)
    ));
    assert!(matches!(
        compare_syntax_condition(
            &condition,
            SyntaxObservation {
                variety: &other,
                ..observation
            }
        ),
        Err(SyntaxConditionRefusal::Variety)
    ));
    let wrong_analysis = LanguageAnalysisRevisionId::new("analysis/other".into()).unwrap();
    assert!(matches!(
        compare_syntax_condition(
            &condition,
            SyntaxObservation {
                analysis: &wrong_analysis,
                ..observation
            }
        ),
        Err(SyntaxConditionRefusal::ForeignArc)
    ));
}

#[test]
fn bounded_condition_conjunction_consumes_the_original_syntax_receipt() {
    use conduit_speech::{
        neighbor_match::NeighborObservation, rule_conditions::*, rule_stress::StressObservation,
    };
    let lexical = lexical("r1");
    let variety = variety("variety/local");
    let analysis = analysis();
    let arc = arc(
        &lexical,
        LanguageUniversalDependencyRelation::Vocative,
        None,
    );
    let conditions = BoundedSequence::try_from_iter([
        SpeechRuleCondition::current_word_has_syntactic_link(LinguisticSyntacticLinkKind::Vocative)
            .unwrap(),
        SpeechRuleCondition::not_careful_style(),
    ])
    .unwrap();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let evidence = ConditionEvidence {
        before: NeighborObservation::Unknown,
        after: NeighborObservation::Absent,
        before_stress: StressObservation::Unknown,
        after_stress: StressObservation::Absent,
        careful_style: &style,
    };
    let observation = SyntaxObservation {
        lexical: &lexical,
        analysis: &analysis,
        arc: &arc,
        ordinal: 1,
        expected_variety: &variety,
        variety: &variety,
    };
    let result = compare_conditions_with_syntax(&conditions, evidence, Some(observation)).unwrap();
    assert_eq!(result.decision(), &SpeechContextDecision::Matched);
    assert_eq!(result.receipts().count(), 2);
    let first = result.receipts().next().unwrap();
    let ConditionReceipt::Syntax(receipt) = first else {
        panic!("original syntax receipt missing")
    };
    assert!(core::ptr::eq(receipt.observation().arc, &arc));
    assert!(matches!(
        compare_conditions(&conditions, evidence),
        Err(ConditionRefusal {
            index: 0,
            reason: ConditionRefusalReason::UnsupportedSyntax
        })
    ));
}

#[test]
fn unknown_legacy_tag_is_refused_by_the_same_language_native_type() {
    use conduit_core::{StructuredInfoRefusal, StructuredInfoValue, StructuredInfoValueShape};
    let known = LinguisticSyntacticLinkKind::Vocative
        .into_structured()
        .unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = known.shape() else {
        panic!("legacy type is a variant")
    };
    assert!(matches!(
        StructuredInfoValue::variant(
            known.value_type().clone(),
            "invented_legacy_subtype",
            payload.clone()
        ),
        Err(StructuredInfoRefusal::UnknownVariantTag)
    ));
}
