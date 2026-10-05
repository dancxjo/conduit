use conduit_core::{projection::*, ArtifactId, BootId, HostId, ImplementationId, PlanId};
use conduit_plot::{ProfileCatalog, StartupCatalog};
use patchbay_graph::*;
fn t(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
struct Unknown;
impl ProjectionDomain for Unknown {
    type Source = &'static str;
    type Target = ();
    type Detail = &'static str;
    fn source_contract(&self) -> ProjectionText<'_> {
        t("external@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        t("portable-subset@1")
    }
    fn validate(
        &self,
        source: &&str,
        target: Option<&()>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        _mechanism: ProjectionMechanism,
    ) -> bool {
        *source == "XYZ"
            && target.is_some()
            && facts.len() == 1
            && native.len() == 1
            && native[0].bytes == source.as_bytes()
            && scores.is_empty()
            && matches!(
                facts[0],
                ProjectionFact::Lost {
                    class: ProjectionLoss::Unrecognized,
                    detail: &"unknown-relation",
                    ..
                }
            )
    }
}
struct Strict;
impl ProjectionPolicy<Unknown> for Strict {
    fn identity(&self) -> ProjectionText<'_> {
        t("strict@1")
    }
    fn permits(&self, _: &&str, _: &(), _: &[ProjectionFact<'_, Unknown>]) -> bool {
        false
    }
}
#[test]
fn exact_checked_graph_inspects_same_report_and_refuses_stale_or_wrong_boundary() {
    let startup = StartupCatalog::new();
    let profile = ProfileCatalog::new();
    let syntax = conduit_plot::parse_syntax_document(
        "plot boundary (\n >> input: U32\n output: U32 >>\n) = (.)\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "boundary", &profile).unwrap();
    let expanded = &authoring.expanded;
    let graph = PatchbayGraph::from_authoring(&authoring).unwrap();
    let selected = graph.subject_ref(&graph.front_inputs[0].identity).unwrap();
    let source = "XYZ";
    let target = ();
    let detail = "unknown-relation";
    let facts = [ProjectionFact::Lost {
        obligation: t("relation/1"),
        class: ProjectionLoss::Unrecognized,
        detail: &detail,
        native_fact: Some(t("native/1")),
    }];
    let native = [ProjectionNativeFact {
        identity: t("native/1"),
        contract: t("external@1"),
        provider: t("fixture@1"),
        encoding: t("utf8@1"),
        bytes: b"XYZ",
    }];
    let plan = PlanId::from("plan/1");
    let host = HostId::from("host/1");
    let boot = BootId::from("boot/1");
    let back = ImplementationId::from("back/1");
    let artifact = ArtifactId::from("artifact/1");
    let admitted = [ProjectionRealization {
        plan: &plan,
        host: &host,
        boot: &boot,
        back: &back,
        artifact: &artifact,
    }];
    let attempts = [ProjectionAttempt {
        realization: admitted[0],
        outcome: ProjectionMechanism::Completed,
        fallback_reason: None,
    }];
    let domain = Unknown;
    let policy = Strict;
    let input = |boundary| ProjectionInput {
        basis: ProjectionBasis {
            report: t("report/1"),
            source: t("external/result/1"),
            target: Some(t("subset/1")),
            projector: t("projection@1"),
            requested_route: t("external@1"),
            boundary,
        },
        source: &source,
        target: Some(&target),
        facts: &facts,
        native: &native,
        scores: &[],
        admitted: &admitted,
        attempts: &attempts,
        selected_attempt: Some(0),
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let report = ProjectionReport::new(
        &domain,
        &policy,
        input(Some(ProjectionBoundary {
            expanded_form: &expanded.expanded_plot_id,
            subject: t(&selected.subject_identity),
        })),
    )
    .unwrap();
    let view = graph.inspect_projection(&selected, &report).unwrap();
    assert!(core::ptr::eq(view.report, &report));
    assert_eq!(
        view.summary.disposition,
        ProjectionDisposition::Insufficient
    );
    assert_eq!(
        view.summary.losses[ProjectionLoss::Unrecognized as usize],
        1
    );
    assert_eq!(view.report.native()[0].bytes, b"XYZ");
    assert!(matches!(
        view.report.facts()[0],
        ProjectionFact::Lost {
            detail: &"unknown-relation",
            ..
        }
    ));
    assert_eq!(
        view.report.selected_attempt().unwrap().realization.back,
        &back
    );
    let mut stale = selected.clone();
    stale.expanded_plot_id = "expanded/stale".into();
    assert!(matches!(
        graph.inspect_projection(&stale, &report),
        Err(PatchbayProjectionRefusal::Graph(
            PatchbayGraphError::StaleGraphBasis
        ))
    ));
    let other = graph.subject_ref(&graph.front_outputs[0].identity).unwrap();
    assert!(matches!(
        graph.inspect_projection(&other, &report),
        Err(PatchbayProjectionRefusal::DifferentBoundary)
    ));
    let unbound = ProjectionReport::new(&domain, &policy, input(None)).unwrap();
    assert!(matches!(
        graph.inspect_projection(&selected, &unbound),
        Err(PatchbayProjectionRefusal::MissingBoundary)
    ));
}
