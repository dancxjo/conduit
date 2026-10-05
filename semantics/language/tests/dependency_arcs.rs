use conduit_language::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
fn token(text: &str, revision: &str, ordinal: u64) -> LanguageAnalysisTokenRef {
    LanguageAnalysisTokenRef::new(
        LanguageAnalysisRevisionId::new(revision.into()).unwrap(),
        LinguisticTokenIdentity::new(ordinal, text.into()).unwrap(),
    )
    .unwrap()
}
fn token_head(value: LanguageAnalysisTokenRef) -> LanguageDependencyHead {
    LanguageDependencyHead::token(value.revision().clone(), value.token().clone()).unwrap()
}
fn relation(base: LanguageUniversalDependencyRelation) -> LanguageDependencyRelation {
    LanguageDependencyRelation::new(base, None).unwrap()
}
#[test]
fn root_and_token_heads_preserve_original_revisioned_endpoints() {
    let dependent = token("source", "analysis/7", 1);
    let root = LanguageDependencyArc::new(
        dependent.clone(),
        LanguageDependencyHead::root(),
        relation(LanguageUniversalDependencyRelation::root()),
    )
    .unwrap();
    assert_eq!(root.dependent(), &dependent);
    let head = token("source", "analysis/7", 0);
    let arc = LanguageDependencyArc::new(
        dependent,
        token_head(head.clone()),
        relation(LanguageUniversalDependencyRelation::vocative()),
    )
    .unwrap();
    assert_eq!(arc.governor(), &token_head(head));
    assert_eq!(
        LanguageDependencyArc::from_structured(arc.clone().into_structured().unwrap()).unwrap(),
        arc
    );
}
#[test]
fn wrong_root_relation_self_source_and_revision_are_native_refusals() {
    let dependent = token("source", "analysis/7", 1);
    for (governor, base) in [
        (
            LanguageDependencyHead::root(),
            LanguageUniversalDependencyRelation::nsubj(),
        ),
        (
            token_head(token("source", "analysis/7", 0)),
            LanguageUniversalDependencyRelation::root(),
        ),
        (
            token_head(dependent.clone()),
            LanguageUniversalDependencyRelation::nsubj(),
        ),
        (
            token_head(token("other source", "analysis/7", 0)),
            LanguageUniversalDependencyRelation::nsubj(),
        ),
        (
            token_head(token("source", "analysis/8", 0)),
            LanguageUniversalDependencyRelation::nsubj(),
        ),
    ] {
        assert!(matches!(
            LanguageDependencyArc::new(dependent.clone(), governor, relation(base)),
            Err(NativeBindingRefusal::ViolatedInvariant { .. })
        ));
    }
    assert!(LanguageAnalysisRevisionId::new("".into()).is_err());
    assert!(LanguageAnalysisRevisionId::new("x".repeat(65)).is_err());
}
#[test]
fn checked_plots_can_name_the_same_native_arc_contract() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            "type ParserProposal = {\n    arc: LanguageDependencyArc\n}\n",
        ),
        &startup,
    );
    assert!(checked.is_ok(), "{checked:?}");
}
