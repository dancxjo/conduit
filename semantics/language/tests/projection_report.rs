//! Supplied external parser records establish fidelity, not parsing accuracy.
use conduit_core::{
    claims::{ClaimScore, ClaimText},
    projection::*,
};
use conduit_language::*;
fn t(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
struct NativeLink {
    relation: &'static str,
    dependent: Option<u64>,
}
struct ParserResult {
    links: [NativeLink; 3],
    cost: i64,
}
#[derive(PartialEq, Eq)]
enum Loss {
    UnknownRelation,
    TokenAlignment,
}
struct ParserProjection;
fn token(ordinal: u64) -> LanguageAnalysisTokenRef {
    LanguageAnalysisTokenRef::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        LinguisticTokenIdentity::new(ordinal, "text/1".into()).unwrap(),
    )
    .unwrap()
}
fn known_arc(dependent: u64) -> LanguageDependencyArc {
    let governor = token(0);
    LanguageDependencyArc::new(
        token(dependent),
        LanguageDependencyHead::token(governor.revision().clone(), governor.token().clone())
            .unwrap(),
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::nsubj(), None)
            .unwrap(),
    )
    .unwrap()
}
fn project(source: &ParserResult) -> Vec<LanguageDependencyArc> {
    source
        .links
        .iter()
        .filter_map(|link| match (link.relation, link.dependent) {
            ("nsubj", Some(n)) if n > 0 && n < 3 => Some(known_arc(n)),
            _ => None,
        })
        .collect()
}
impl ProjectionDomain for ParserProjection {
    type Source = ParserResult;
    type Target = Vec<LanguageDependencyArc>;
    type Detail = Loss;
    fn source_contract(&self) -> ProjectionText<'_> {
        t("external-parser/result@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        t("language/dependency-arcs@1")
    }
    fn validate(
        &self,
        source: &ParserResult,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        _mechanism: ProjectionMechanism,
    ) -> bool {
        if facts.len() != 3 || native.len() != 4 || scores.len() != 1 || source.cost != 17 {
            return false;
        }
        let Some(target) = target else {
            return false;
        };
        if target.len() != 1 || target[0] != known_arc(1) {
            return false;
        }
        let names = ["link/0", "link/1", "link/2"];
        for (i, link) in source.links.iter().enumerate() {
            if facts[i].obligation().as_str() != names[i]
                || native[i].identity.as_str() != names[i]
                || native[i].bytes != link.relation.as_bytes()
                || native[i].provider.as_str() != "parser/fixture@1"
            {
                return false;
            }
            match (link.relation, link.dependent, &facts[i]) {
                ("nsubj", Some(1), ProjectionFact::Preserved { .. }) => (),
                (
                    "XYZ",
                    Some(2),
                    ProjectionFact::Lost {
                        class: ProjectionLoss::Unrecognized,
                        detail: Loss::UnknownRelation,
                        native_fact: Some(id),
                        ..
                    },
                ) if id.as_str() == names[i] => (),
                (
                    "nsubj",
                    None,
                    ProjectionFact::Lost {
                        class: ProjectionLoss::Identity,
                        detail: Loss::TokenAlignment,
                        native_fact: Some(id),
                        ..
                    },
                ) if id.as_str() == names[i] => (),
                _ => return false,
            }
        }
        matches!(scores[0],ProjectionScore::Native {fact,score} if fact.as_str()=="cost" && score.value()==source.cost && score.scale().as_str()=="parser/integer-cost@1" && score.calibration().is_none())
            && native[3].bytes == b"17"
    }
}
struct Strict;
impl ProjectionPolicy<ParserProjection> for Strict {
    fn identity(&self) -> ProjectionText<'_> {
        t("strict-language@1")
    }
    fn permits(
        &self,
        _: &ParserResult,
        _: &Vec<LanguageDependencyArc>,
        _: &[ProjectionFact<'_, ParserProjection>],
    ) -> bool {
        false
    }
}
#[test]
fn external_unknown_alignment_and_cost_remain_visible_and_strict_refuses() {
    let source = ParserResult {
        links: [
            NativeLink {
                relation: "nsubj",
                dependent: Some(1),
            },
            NativeLink {
                relation: "XYZ",
                dependent: Some(2),
            },
            NativeLink {
                relation: "nsubj",
                dependent: None,
            },
        ],
        cost: 17,
    };
    let target = project(&source);
    let unknown = Loss::UnknownRelation;
    let alignment = Loss::TokenAlignment;
    let facts = [
        ProjectionFact::Preserved {
            obligation: t("link/0"),
        },
        ProjectionFact::Lost {
            obligation: t("link/1"),
            class: ProjectionLoss::Unrecognized,
            detail: &unknown,
            native_fact: Some(t("link/1")),
        },
        ProjectionFact::Lost {
            obligation: t("link/2"),
            class: ProjectionLoss::Identity,
            detail: &alignment,
            native_fact: Some(t("link/2")),
        },
    ];
    let native = [
        ("link/0", b"nsubj".as_slice()),
        ("link/1", b"XYZ".as_slice()),
        ("link/2", b"nsubj".as_slice()),
        ("cost", b"17".as_slice()),
    ]
    .map(|(identity, bytes)| ProjectionNativeFact {
        identity: t(identity),
        contract: t("parser/native-record@1"),
        provider: t("parser/fixture@1"),
        encoding: t("utf8@1"),
        bytes,
    });
    let cost = ClaimScore::new(
        ClaimText::new("parser/integer-cost@1").unwrap(),
        None,
        ClaimText::new("parser/fixture@1").unwrap(),
        0,
        100,
        17,
    )
    .unwrap();
    let scores = [ProjectionScore::Native {
        fact: t("cost"),
        score: cost,
    }];
    let domain = ParserProjection;
    let policy = Strict;
    let input = |target, facts| ProjectionInput {
        basis: ProjectionBasis {
            report: t("report/parser/1"),
            source: t("parser/result/1"),
            target: Some(t("arcs/1")),
            projector: t("parser-to-language@1"),
            requested_route: t("supplied-parser@1"),
            boundary: None,
        },
        source: &source,
        target: Some(target),
        facts,
        native: &native,
        scores: &scores,
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let report = ProjectionReport::new(&domain, &policy, input(&target, &facts)).unwrap();
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Insufficient
    );
    assert_eq!(report.summary().preserved, 1);
    assert_eq!(
        report.summary().losses[ProjectionLoss::Unrecognized as usize],
        1
    );
    assert_eq!(
        report.summary().losses[ProjectionLoss::Identity as usize],
        1
    );
    assert_eq!(report.native()[1].bytes, b"XYZ");
    assert_eq!(
        report.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert_eq!(report.inspect_target().unwrap()[0], known_arc(1));
    let confidence = ClaimScore::new(
        ClaimText::new("probability/percent@1").unwrap(),
        None,
        ClaimText::new("parser/fixture@1").unwrap(),
        0,
        100,
        17,
    )
    .unwrap();
    assert!(cost.compare(&confidence).is_err());
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&target, &facts[..2])),
        Err(ProjectionRefusal::Domain)
    ));
    // Coercing the unknown external link into any portable arc is rejected.
    let mut invented = target.clone();
    invented.push(known_arc(2));
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&invented, &facts)),
        Err(ProjectionRefusal::Domain)
    ));
}
