use conduit_core::revision::*;
use conduit_language::{discourse::*, revision::TokenFrontier, *};
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "supplied-ud-fixture".into(),
        "fixture/1".into(),
    )
    .unwrap()
}
fn source(text: &str) -> LanguageTextRevision {
    LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("r1".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap()
}
fn analysis() -> LanguageAnalysisRevisionId {
    LanguageAnalysisRevisionId::new("a1".into()).unwrap()
}
fn arc(
    dependent: u64,
    head: u64,
    relation: LanguageUniversalDependencyRelation,
) -> LanguageDependencyArc {
    let token = |ordinal| {
        LinguisticTokenIdentity::new(
            ordinal,
            LanguageTextId::new("text".into()).unwrap(),
            LanguageTextRevisionId::new("r1".into()).unwrap(),
        )
        .unwrap()
    };
    LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(analysis(), token(dependent)).unwrap(),
        LanguageDependencyHead::token(analysis(), token(head)).unwrap(),
        LanguageDependencyRelation::new(relation, None).unwrap(),
    )
    .unwrap()
}
#[test]
fn initial_medial_and_final_vocatives_follow_exact_edges_without_punctuation() {
    for (text, dependent, head) in [
        ("Travis hello friend", 0, 1),
        ("hello Travis friend", 1, 0),
        ("hello friend Travis", 2, 0),
    ] {
        let source = source(text);
        let arc = arc(
            dependent,
            head,
            LanguageUniversalDependencyRelation::Vocative,
        );
        let prepared =
            prepare_vocative_fact("fact".into(), &source, &analysis(), &arc, provenance(), 3)
                .unwrap();
        assert!(matches!(
            prepared.fact().role(),
            LanguageDiscourseRole::Addressee
        ));
        assert_eq!(prepared.fact().basis(), &arc);
        assert_eq!(prepared.fact().source(), &source);
        assert_eq!(
            *prepared.fact().basis().dependent().token().ordinal(),
            dependent
        );
    }
}
#[test]
fn nonvocative_wrong_source_analysis_and_bounds_refuse_before_derivation() {
    let source = source("hello Travis friend");
    let edge = arc(1, 0, LanguageUniversalDependencyRelation::Vocative);
    let foreign_analysis = LanguageAnalysisRevisionId::new("a2".into()).unwrap();
    assert!(matches!(
        prepare_vocative_fact(
            "fact".into(),
            &source,
            &foreign_analysis,
            &edge,
            provenance(),
            3
        ),
        Err(DiscourseRefusal::Native(_))
    ));
    let object = arc(1, 0, LanguageUniversalDependencyRelation::Obj);
    assert!(matches!(
        prepare_vocative_fact(
            "fact".into(),
            &source,
            &analysis(),
            &object,
            provenance(),
            3
        ),
        Err(DiscourseRefusal::Native(_))
    ));
    assert!(LanguageVocativeDiscourseFact::new(
        analysis(),
        object.clone(),
        "forged".into(),
        provenance(),
        LanguageDiscourseRole::Addressee,
        source.clone()
    )
    .is_err());
    let foreign_revision = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            source.material().identity().clone(),
            source.material().language().clone(),
            LanguageTextRevisionId::new("r2".into()).unwrap(),
            source.material().text().clone(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap();
    assert!(matches!(
        prepare_vocative_fact(
            "fact".into(),
            &foreign_revision,
            &analysis(),
            &edge,
            provenance(),
            3
        ),
        Err(DiscourseRefusal::Native(_))
    ));
    let foreign = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("foreign".into()).unwrap(),
            source.material().language().clone(),
            source.material().revision().clone(),
            "hello Travis friend".into(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap();
    assert!(matches!(
        prepare_vocative_fact("fact".into(), &foreign, &analysis(), &edge, provenance(), 3),
        Err(DiscourseRefusal::Native(_))
    ));
    assert!(matches!(
        prepare_vocative_fact("fact".into(), &source, &analysis(), &edge, provenance(), 1),
        Err(DiscourseRefusal::TokenBound)
    ));
}
#[test]
fn discourse_schemas_are_installed_for_ordinary_authored_plots() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    for (name, _) in discourse_types() {
        let source = format!(
            "plot language/discourse-fixture (\n >> value: {name}\n result: {name} >>\n) = (.)"
        );
        conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
    }
}

#[test]
fn ledger_distinguishes_stability_withdrawal_and_commitment() {
    let source = source("hello Travis friend");
    let edge = arc(1, 0, LanguageUniversalDependencyRelation::Vocative);
    let fact =
        prepare_vocative_fact("fact".into(), &source, &analysis(), &edge, provenance(), 3).unwrap();
    let domain = DiscourseRevisions {
        source: &source,
        subject: edge.dependent().token(),
        tokens: 3,
    };
    let text = |value| RevisionText::new(value).unwrap();
    let context = RevisionContext {
        stream: text("discourse/1"),
        subject: text("token/1"),
        epoch: text("text/r1"),
        producer: text("language/vocative-discourse"),
        policy: text("finite/1"),
    };
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let evidence = [RevisionEvidence {
        source: text("syntax"),
        generation: text("a1"),
    }];
    let asserted = DiscourseDelta::Assert(fact.fact());
    let withdrawal = DiscourseDelta::Withdraw(edge.dependent().token());
    let proposed = RevisionEvent::new(
        &domain,
        reference(1, "proposal"),
        &evidence,
        RevisionChange::Proposed { delta: &asserted },
    )
    .unwrap();
    let stable = RevisionEvent::new(
        &domain,
        reference(2, "stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let withdrawn = RevisionEvent::new(
        &domain,
        reference(3, "withdrawn"),
        &evidence,
        RevisionChange::Withdrawn {
            revision: proposed.reference(),
            delta: &withdrawal,
            reason: text("syntax hypothesis withdrawn"),
        },
    )
    .unwrap();
    let other_edge = arc(2, 0, LanguageUniversalDependencyRelation::Vocative);
    let other_withdrawal = DiscourseDelta::Withdraw(other_edge.dependent().token());
    assert!(RevisionEvent::new(
        &domain,
        reference(3, "wrong-subject"),
        &evidence,
        RevisionChange::Withdrawn {
            revision: proposed.reference(),
            delta: &other_withdrawal,
            reason: text("wrong occurrence")
        }
    )
    .is_err());
    let mut journal = RevisionJournal::new(
        &domain,
        context,
        TokenFrontier(0),
        RevisionLimits {
            history_events: 4,
            revisable_units: 3,
        },
    )
    .unwrap();
    journal.append(&proposed).unwrap();
    journal.append(&stable).unwrap();
    assert_eq!(journal.frontiers().stable_through, Some(TokenFrontier(2)));
    assert_eq!(journal.frontiers().committed, TokenFrontier(0));
    journal.append(&withdrawn).unwrap();
    assert_eq!(journal.frontiers().stable_through, None);
    assert_eq!(journal.frontiers().committed, TokenFrontier(0));
    let history: Vec<_> = journal.history().collect();
    assert_eq!(history.len(), 3);
    assert!(matches!(
        history[2].change(),
        RevisionChange::Withdrawn { .. }
    ));
    assert_eq!(fact.fact().basis(), &edge);
}
