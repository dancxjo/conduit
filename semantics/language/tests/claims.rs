//! #4907 consumes core without moving dependency ontology or parser policy there.
use conduit_core::claims::*;
use conduit_language::*;

fn text(s: &str) -> ClaimText<'_> {
    ClaimText::new(s).unwrap()
}
fn reason() -> ClaimReason<'static> {
    ClaimReason {
        profile: text("language/plausible-alternatives@1"),
        explanation: text(
            "The supplied parser alternatives disagree; retain shared endpoints only",
        ),
    }
}
struct Syntax;
impl ClaimDomain for Syntax {
    type Target = LanguageAnalysisTokenRef;
    type Value = LanguageDependencyArc;
    fn contract(&self) -> ClaimText<'_> {
        text("language/dependency-assertion@1")
    }
    fn validate(&self, target: &Self::Target, value: &Self::Value) -> bool {
        value.dependent() == target
    }
}
struct PlausibleParses;
impl ClaimPolicy<Syntax> for PlausibleParses {
    fn identity(&self) -> ClaimText<'_> {
        text("language/conservative-parser-fixture@1")
    }
    fn assess<'a>(
        &self,
        _: &SemanticClaim<'a, Syntax>,
        _: &[&SemanticClaim<'a, Syntax>],
    ) -> Option<ClaimReason<'a>> {
        None
    }
    fn decide<'a>(
        &self,
        _: &[&SemanticClaim<'a, Syntax>],
        _: &[bool],
        _: &[&SemanticClaim<'a, Syntax>],
    ) -> ClaimDecision<'a> {
        ClaimDecision::Abstained { reason: reason() }
    }
}
fn token(ordinal: u64) -> LanguageAnalysisTokenRef {
    LanguageAnalysisTokenRef::new(
        LanguageAnalysisRevisionId::new("analysis/7".into()).unwrap(),
        LinguisticTokenIdentity::new(
            ordinal,
            LanguageTextId::new("text/revision/3".into()).unwrap(),
            LanguageTextRevisionId::new("source/3".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn arc(base: LanguageUniversalDependencyRelation) -> LanguageDependencyArc {
    let head = token(0);
    LanguageDependencyArc::new(
        token(1),
        LanguageDependencyHead::token(head.revision().clone(), head.token().clone()).unwrap(),
        LanguageDependencyRelation::new(base, None).unwrap(),
    )
    .unwrap()
}
#[test]
fn exact_revisioned_parser_alternatives_abstain_and_share_only_common_facts() {
    let target = token(1);
    let vocative = arc(LanguageUniversalDependencyRelation::vocative());
    let apposition = arc(LanguageUniversalDependencyRelation::appos());
    let support = [ClaimSupport::SourceGeneration {
        source: text("text/utterance"),
        generation: text("text/revision/3"),
    }];
    let conflicts_a = [text("parse/b")];
    let conflicts_b = [text("parse/a")];
    let basis = |identity, conflicts| ClaimBasis {
        identity: text(identity),
        producer: text("parser/back@7"),
        artifact: text("parser/model-digest/17"),
        support: &support,
        conflicts,
        score: None,
        rationale: reason(),
    };
    let domain = Syntax;
    let mut a =
        SemanticClaim::new(&domain, &target, &vocative, basis("parse/a", &conflicts_a)).unwrap();
    let b = SemanticClaim::new(
        &domain,
        &target,
        &apposition,
        basis("parse/b", &conflicts_b),
    )
    .unwrap();
    a.transition(ClaimChange {
        state: ClaimLifecycle::Stable,
        reason: reason(),
        replacement: None,
    })
    .unwrap();
    {
        let candidates = [&a, &b];
        let policy = PlausibleParses;
        let result = resolve_claims(text("language/resolution/7"), &candidates, &policy).unwrap();
        let replay = resolve_claims(text("language/resolution/7"), &candidates, &policy).unwrap();
        assert_eq!(result.decision(), replay.decision());
        assert_eq!(result.receipts(), replay.receipts());
        assert!(matches!(result.decision(), ClaimDecision::Abstained { .. }));
        let plausible: Vec<_> = result
            .candidates()
            .iter()
            .zip(result.receipts())
            .filter(|(_, receipt)| receipt.unwrap().exclusion.is_none())
            .map(|(claim, _)| claim.value())
            .collect();
        // The domain computes intersection; core does not know dependency semantics.
        assert!(plausible.iter().all(|arc| arc.dependent() == &target));
        assert!(plausible
            .iter()
            .all(|arc| arc.governor() == plausible[0].governor()));
        assert!(!plausible
            .iter()
            .all(|arc| arc.relation() == plausible[0].relation()));
        assert_eq!(
            result.candidates()[0].basis().artifact.as_str(),
            "parser/model-digest/17"
        );
    }
    a.transition(ClaimChange {
        state: ClaimLifecycle::Revised,
        reason: reason(),
        replacement: Some(text("parse/b")),
    })
    .unwrap();
    let candidates = [&a, &b];
    let result =
        resolve_claims(text("language/resolution/8"), &candidates, &PlausibleParses).unwrap();
    assert!(result.receipts()[0].unwrap().exclusion.is_some());
    assert_eq!(result.candidates()[0].value(), &vocative);
    assert_eq!(result.candidates()[0].history().len(), 2);
}
