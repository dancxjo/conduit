//! Actual linear presenter evidence, independent of physical/human enactment.
use conduit_core::{kind_id, projection::*};
use conduit_presentation::{
    render_linear_presentation, LinearPresentation, Presentation, PresentationAction,
    PresentationActionAvailability, PresentationBasis, PresentationCompositionKind,
    PresentationCompositionRelation, PresentationDisclosureLevel, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
};
fn t(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn face() -> Presentation {
    Presentation::new_with_semantics(
        1,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            PresentationSubject {
                identity: "source".into(),
                name: "Archive".into(),
                role: PresentationRole::Document,
            },
            PresentationSubject {
                identity: "report".into(),
                name: "Annual report".into(),
                role: PresentationRole::Document,
            },
        ],
        vec![PresentationRelationship {
            source: "source".into(),
            target: "report".into(),
            kind: PresentationRelationshipKind::Semantic(kind_id("reading/precedes@1")),
        }],
        vec![],
        vec![PresentationText {
            subject: "report".into(),
            text: "Accessible report description".into(),
        }],
        vec![PresentationAction {
            identity: "copy-report".into(),
            intent: "file/copy@1".into(),
            target: "report".into(),
            name: "Copy report".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
    .with_composition(vec![PresentationCompositionRelation {
        identity: "group/report-with-source".into(),
        source: "report".into(),
        target: "source".into(),
        kind: PresentationCompositionKind::Group,
    }])
    .unwrap()
}
struct LinearProjection<'a> {
    rendered: &'a LinearPresentation,
}
impl ProjectionDomain for LinearProjection<'_> {
    type Source = Presentation;
    type Target = LinearPresentation;
    type Detail = ();
    fn source_contract(&self) -> ProjectionText<'_> {
        t("presentation/face@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        t("presentation/linear-records@1")
    }
    fn validate(
        &self,
        source: &Presentation,
        target: Option<&LinearPresentation>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        _mechanism: ProjectionMechanism,
    ) -> bool {
        let names = [
            "subject/source",
            "subject/report",
            "relationship/order",
            "composition/group",
            "text/report",
            "action/copy-report",
        ];
        source.validate().is_ok()
            && source.subjects.len() == 2
            && source.relationships.len() == 1
            && source.composition.len() == 1
            && source.text.len() == 1
            && source.actions.len() == 1
            && source.properties.is_empty()
            && source.disclosures.is_empty()
            && source.temporal_references.is_empty()
            && source.temporal_facts.is_empty()
            && self.rendered.presentation_id == source.identity
            && self.rendered.revision == source.revision
            && target == Some(self.rendered)
            && native.is_empty()
            && scores.is_empty()
            && facts.len() == names.len()
            && facts
                .iter()
                .zip(names)
                .all(|(f, name)| f.obligation().as_str() == name)
            && facts.iter().enumerate().all(|(i, f)| match f {
                ProjectionFact::Transformed { law, .. } => {
                    i == 3 && law.as_str() == "group-to-linear-record@1"
                }
                ProjectionFact::Preserved { .. } => i != 3,
                _ => false,
            })
    }
}
struct Strict;
impl ProjectionPolicy<LinearProjection<'_>> for Strict {
    fn identity(&self) -> ProjectionText<'_> {
        t("face/semantic-strict@1")
    }
    fn permits(
        &self,
        _: &Presentation,
        _: &LinearPresentation,
        _: &[ProjectionFact<'_, LinearProjection<'_>>],
    ) -> bool {
        false
    }
}
#[test]
fn linearization_preserves_accessible_meaning_and_reports_transformation() {
    let source = face();
    let target = render_linear_presentation(&source).unwrap();
    let records = target.lines.join("\n");
    for exact in [
        "Archive",
        "Annual report",
        "Accessible report description",
        "Copy report",
        "file/copy@1",
        "reading/precedes@1",
        "group/report-with-source",
        "Group",
    ] {
        assert!(records.contains(exact), "missing {exact}: {records}");
    }
    let facts = [
        ProjectionFact::Preserved {
            obligation: t("subject/source"),
        },
        ProjectionFact::Preserved {
            obligation: t("subject/report"),
        },
        ProjectionFact::Preserved {
            obligation: t("relationship/order"),
        },
        ProjectionFact::Transformed {
            obligation: t("composition/group"),
            law: t("group-to-linear-record@1"),
        },
        ProjectionFact::Preserved {
            obligation: t("text/report"),
        },
        ProjectionFact::Preserved {
            obligation: t("action/copy-report"),
        },
    ];
    let domain = LinearProjection { rendered: &target };
    let policy = Strict;
    let input = |target, facts| ProjectionInput {
        basis: ProjectionBasis {
            report: t("report/linear/1"),
            source: t(source.identity.as_str()),
            target: Some(t("artifact/linear-specimen/1")),
            projector: t("linear-presenter@1"),
            requested_route: t("presentation/linear@1"),
            boundary: None,
        },
        source: &source,
        target: Some(target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let report = ProjectionReport::new(&domain, &policy, input(&target, &facts)).unwrap();
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::Exact)
    );
    assert_eq!(
        (report.summary().preserved, report.summary().transformed),
        (5, 1)
    );
    assert_eq!(report.require_exact().unwrap(), &target);
    assert_ne!(report.basis().source, report.basis().target.unwrap());
    // Losing an action is accidental semantic loss, not target-appropriate layout.
    let mut omitted = target.clone();
    omitted.lines.retain(|line| !line.contains("copy-report"));
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&omitted, &facts)),
        Err(ProjectionRefusal::Domain)
    ));
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, input(&target, &facts[..5])),
        Err(ProjectionRefusal::Domain)
    ));
}
