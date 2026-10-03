use conduit_body::{Body, BodyFulfillment, FulfillmentObligation, Wake};
use conduit_core::{
    bind_active_play, kind_id, seal_plan, AuthorityGrantId, CheckedPlotId, ExpandedPlotId,
    PlotIdentity, SignId, SourceDocumentId,
};
use conduit_presentation::{
    render_linear_presentation, Face, FaceContext, FaceContribution, FaceContributionRole,
    FaceFocus, FaceNames, FaceOperatorActionKind, FaceRefusal, FaceResidentPlotName,
    GenerativeNarratorRole, GenerativePresenterBounds, GenerativePresenterPolicy,
    GenerativePresenterRequest, NavigationAspect, NavigationPlace, PresentationAction,
    PresentationActionAvailability, PresentationAspect, PresentationCompositionKind,
    PresentationCompositionRelation, PresentationContributionBasis, PresentationCursor,
    PresentationDepth, PresentationDisclosure, PresentationDisclosureLevel, PresentationFragment,
    PresentationNavigation, PresentationPlace, PresentationProjection, PresentationPropertyValue,
    PresentationRole, PresentationSubject, ProjectionItem, ProjectionMembership,
};

fn born_body() -> Body {
    Body::born(
        SourceDocumentId::from("source/tutorial"),
        CheckedPlotId::from("checked/tutorial"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
}

#[test]
fn owner_names_are_human_facing_while_exact_body_and_plot_truth_remain_inspectable() {
    let body = born_body();
    let resident = &body.workset.plots()[0];
    let plot_names = [FaceResidentPlotName {
        source_document_id: &resident.source_document_id,
        checked_plot_id: &resident.checked_plot_id,
        name: "Field Station Clock",
    }];
    let face = Face::project_with_names(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
        FaceNames {
            body_name: Some("North Station"),
            resident_plots: &plot_names,
        },
    )
    .unwrap();

    let body_subject = face
        .presentation
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Body)
        .unwrap();
    let plot_subject = face
        .presentation
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Plot)
        .unwrap();
    assert_eq!(body_subject.name, "North Station");
    assert_eq!(plot_subject.name, "Field Station Clock");
    assert_eq!(
        body_subject.identity,
        format!("body/{}", body.body_id.as_str())
    );
    assert_eq!(
        plot_subject.identity,
        format!("plot/{}", resident.checked_plot_id.as_str())
    );
    assert!(face.presentation.properties.iter().any(|property| {
        property.subject == plot_subject.identity
            && property.name == "checked-plot-id"
            && property.value
                == PresentationPropertyValue::Identity(resident.checked_plot_id.as_str().into())
    }));
    assert!(face.presentation.text.iter().any(|text| {
        text.subject == body_subject.identity
            && text.text == "North Station is lulled with one resident plot."
    }));
    face.presentation.validate().unwrap();
}

#[test]
fn owner_names_refuse_stale_or_ambiguous_plot_labels() {
    let body = born_body();
    let resident = &body.workset.plots()[0];
    let stale_checked = CheckedPlotId::from("checked/old-plot");
    let stale = [FaceResidentPlotName {
        source_document_id: &resident.source_document_id,
        checked_plot_id: &stale_checked,
        name: "Old clock",
    }];
    let project = |names| {
        Face::project_with_names(
            &body,
            None,
            7,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
            names,
        )
    };
    assert_eq!(
        project(FaceNames {
            body_name: None,
            resident_plots: &stale,
        }),
        Err(FaceRefusal::PlotNameNotResident)
    );
    let stale_source = SourceDocumentId::from("source/old-plot");
    let wrong_source = [FaceResidentPlotName {
        source_document_id: &stale_source,
        checked_plot_id: &resident.checked_plot_id,
        name: "Old clock",
    }];
    assert_eq!(
        project(FaceNames {
            body_name: None,
            resident_plots: &wrong_source,
        }),
        Err(FaceRefusal::PlotNameNotResident)
    );
    let valid = FaceResidentPlotName {
        source_document_id: &resident.source_document_id,
        checked_plot_id: &resident.checked_plot_id,
        name: "Clock",
    };
    let duplicated = [valid, valid];
    assert_eq!(
        project(FaceNames {
            body_name: None,
            resident_plots: &duplicated,
        }),
        Err(FaceRefusal::DuplicatePlotName)
    );
    assert_eq!(
        project(FaceNames {
            body_name: Some("  "),
            resident_plots: &[],
        }),
        Err(FaceRefusal::InvalidBodyName)
    );
}

fn playing() -> (Body, Wake, conduit_core::PlanId, conduit_core::ActivePlayId) {
    let (body, wake) = born_body().wake(2, SignId::from("sign/woke")).unwrap();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: SourceDocumentId::from("source/tutorial"),
            checked_plot_id: CheckedPlotId::from("checked/tutorial"),
            expanded_plot_id: ExpandedPlotId::from("expanded/tutorial"),
        },
        vec![],
    );
    let wake = wake
        .plan_ready(&plan, SignId::from("sign/planned"))
        .unwrap();
    let play = bind_active_play(
        &plan.plan_id,
        &"host/tutorial".into(),
        &"boot/tutorial".into(),
        3,
    );
    let play_id = play.active_play_id.clone();
    let wake = wake
        .play_started(&play, SignId::from("sign/playing"))
        .unwrap();
    (body, wake, plan.plan_id, play_id)
}

fn tutorial_fragment(
    plan_id: conduit_core::PlanId,
    play_id: conduit_core::ActivePlayId,
) -> PresentationFragment {
    PresentationFragment {
        basis: PresentationContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/tutorial"),
            plan_id,
            active_play_id: play_id,
            required_interaction_context: None,
        },
        subjects: vec![
            PresentationSubject {
                identity: "tutorial".into(),
                role: PresentationRole::Semantic(kind_id("education/tutorial")),
                name: "Learn this body".into(),
            },
            PresentationSubject {
                identity: "continue".into(),
                role: PresentationRole::Action,
                name: "Continue".into(),
            },
        ],
        relationships: vec![],
        composition: vec![],
        properties: vec![],
        text: vec![],
        actions: vec![PresentationAction {
            identity: "tutorial.continue".into(),
            intent: "education/tutorial/continue@1".into(),
            target: "continue".into(),
            name: "Continue".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    }
}

#[test]
fn lulled_body_has_an_exact_surface_without_a_running_plot() {
    let body = born_body();
    let surface = Face::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();

    surface.presentation.validate().unwrap();
    assert_eq!(surface.presentation.basis.body_id, Some(body.body_id));
    assert_eq!(surface.presentation.basis.wake_id, None);
    assert_eq!(surface.presentation.revision, 7);
    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "lifecycle-state"
            && property.value == PresentationPropertyValue::Text("lulled".into())
    }));
    assert!(surface.operator_actions.iter().any(|action| {
        action.kind == FaceOperatorActionKind::Wake
            && surface
                .resolve_operator_action(7, &action.surface_action_id)
                .is_ok()
    }));
}

#[test]
fn resident_view_joins_body_truth_only_for_its_current_play() {
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution::from_presentation(
        FaceContributionRole::Tutorial,
        tutorial_fragment(plan_id.clone(), play_id.clone()),
    );
    let surface = Face::project(
        &body,
        Some(&wake),
        20,
        FaceContext::Tutorial(CheckedPlotId::from("checked/tutorial")),
        FaceFocus::Contribution {
            role: FaceContributionRole::Tutorial,
            node_key: Some("continue".into()),
        },
        vec![contribution.clone()],
    )
    .unwrap();

    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "active-play-id"
            && property.value == PresentationPropertyValue::Identity(play_id.as_str().into())
    }));
    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "plan-id"
            && property.value == PresentationPropertyValue::Identity(plan_id.as_str().into())
    }));
    assert!(surface
        .presentation
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Plan));
    assert!(surface
        .presentation
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Play));
    let action = surface
        .presentation
        .actions
        .iter()
        .find(|action| action.name == "Continue")
        .unwrap();
    assert!(surface
        .presentation
        .resolve_action(20, &action.identity)
        .is_ok());
    let library = surface
        .operator_actions
        .iter()
        .find(|action| action.kind == FaceOperatorActionKind::OpenLibrary)
        .unwrap();
    assert!(surface
        .resolve_operator_action(20, &library.surface_action_id)
        .is_ok());
    assert_eq!(
        surface.resolve_operator_action(19, &library.surface_action_id),
        Err(FaceRefusal::StaleAction)
    );
    let inspection = surface
        .operator_actions
        .iter()
        .find(|action| {
            action.kind
                == FaceOperatorActionKind::OpenInspection(CheckedPlotId::from("checked/tutorial"))
        })
        .unwrap();
    assert_eq!(
        surface.resolve_operator_action(20, &inspection.surface_action_id),
        Err(FaceRefusal::UnavailableAction)
    );

    let lulled = born_body();
    assert_eq!(
        Face::project(
            &lulled,
            None,
            21,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![contribution],
        ),
        Err(FaceRefusal::PlayNotCurrent)
    );
}

#[test]
fn presentation_only_navigation_does_not_change_body_or_running_work() {
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution::from_presentation(
        FaceContributionRole::Foreground,
        tutorial_fragment(plan_id, play_id),
    );
    let overview = Face::project(
        &body,
        Some(&wake),
        1,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![contribution.clone()],
    )
    .unwrap();
    let foreground = Face::project(
        &body,
        Some(&wake),
        2,
        FaceContext::ResidentPlot(CheckedPlotId::from("checked/tutorial")),
        FaceFocus::Contribution {
            role: FaceContributionRole::Foreground,
            node_key: None,
        },
        vec![contribution],
    )
    .unwrap();

    assert_eq!(
        overview.presentation.basis.body_id,
        foreground.presentation.basis.body_id
    );
    assert_eq!(
        overview.presentation.basis.wake_id,
        foreground.presentation.basis.wake_id
    );
    assert_eq!(body.workload_revision, 0);
    assert_eq!(wake.plans.len(), 1);
    assert_ne!(
        overview.presentation.identity,
        foreground.presentation.identity
    );
}

#[test]
fn composition_is_finite_and_deterministic() {
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution::from_presentation(
        FaceContributionRole::Tutorial,
        tutorial_fragment(plan_id, play_id),
    );
    let project = || {
        Face::project(
            &body,
            Some(&wake),
            31,
            FaceContext::Tutorial(CheckedPlotId::from("checked/tutorial")),
            FaceFocus::Body,
            vec![contribution.clone()],
        )
        .unwrap()
    };
    let first = project();
    let repeat = project();
    assert_eq!(first.presentation.identity, repeat.presentation.identity);
    assert_eq!(
        Face::project(
            &body,
            Some(&wake),
            32,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![contribution.clone(), contribution],
        ),
        Err(FaceRefusal::DuplicateRole)
    );
}

fn minimal_fragment(
    plan_id: conduit_core::PlanId,
    play_id: conduit_core::ActivePlayId,
    required_context: Option<&str>,
    subject_identity: String,
) -> PresentationFragment {
    PresentationFragment {
        basis: PresentationContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/tutorial"),
            plan_id,
            active_play_id: play_id,
            required_interaction_context: required_context.map(Into::into),
        },
        subjects: vec![PresentationSubject {
            identity: subject_identity,
            role: PresentationRole::Semantic(kind_id("education/concept")),
            name: "Concept".into(),
        }],
        relationships: vec![],
        composition: vec![],
        properties: vec![],
        text: vec![],
        actions: vec![],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    }
}

#[test]
fn direct_contributions_cannot_replace_face_truth_or_privately_choose_context() {
    let (body, wake, plan_id, play_id) = playing();
    let project = |fragment| {
        Face::project(
            &body,
            Some(&wake),
            31,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![FaceContribution::from_presentation(
                FaceContributionRole::Foreground,
                fragment,
            )],
        )
    };

    let body_identity = format!("body/{}", body.body_id.as_str());
    assert_eq!(
        project(minimal_fragment(
            plan_id.clone(),
            play_id.clone(),
            None,
            body_identity.clone(),
        )),
        Err(FaceRefusal::FaceOwnedIdentity(body_identity))
    );
    assert_eq!(
        project(minimal_fragment(
            plan_id,
            play_id,
            Some("face/context/private-audience"),
            "concept/private".into(),
        )),
        Err(FaceRefusal::IncompatibleInteractionContext)
    );
}

#[test]
fn ordinary_plot_contributes_universal_truth_without_an_application_view() {
    let (body, wake, plan_id, play_id) = playing();
    let context_id = "face/context/tutorial/checked/tutorial/focus/contribution/tutorial/node/concept/mitochondrion";
    let contribution = FaceContribution {
        role: FaceContributionRole::Tutorial,
        checked_plot_id: CheckedPlotId::from("checked/tutorial"),
        plan_id: plan_id.clone(),
        active_play_id: play_id.clone(),
        presentation: Box::new(PresentationFragment {
            basis: PresentationContributionBasis {
                checked_plot_id: CheckedPlotId::from("checked/tutorial"),
                plan_id,
                active_play_id: play_id,
                required_interaction_context: Some(context_id.into()),
            },
            subjects: vec![
                PresentationSubject {
                    identity: "lesson/cell".into(),
                    role: PresentationRole::Semantic(kind_id("education/lesson")),
                    name: "The cell".into(),
                },
                PresentationSubject {
                    identity: "concept/mitochondrion".into(),
                    role: PresentationRole::Semantic(kind_id("biology/cell/organelle")),
                    name: "Mitochondrion".into(),
                },
            ],
            relationships: vec![],
            composition: vec![PresentationCompositionRelation {
                identity: "composition/emphasize-mitochondrion".into(),
                source: "concept/mitochondrion".into(),
                target: "lesson/cell".into(),
                kind: PresentationCompositionKind::Emphasize,
            }],
            properties: vec![],
            text: vec![],
            actions: vec![PresentationAction {
                identity: "lesson/answer".into(),
                intent: "education/answer@1".into(),
                target: "concept/mitochondrion".into(),
                name: "Answer".into(),
                arguments: vec![conduit_presentation::FaceActionArgument::text(
                    "lesson/answer-text".into(),
                    "Answer".into(),
                    1,
                    128,
                )
                .unwrap()],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            }],
            disclosures: vec![PresentationDisclosure {
                subject: "concept/mitochondrion".into(),
                level: PresentationDisclosureLevel::Primary,
            }],
            temporal_references: vec![],
            temporal_facts: vec![],
        }),
    };
    let face = Face::project(
        &body,
        Some(&wake),
        30,
        FaceContext::Tutorial(CheckedPlotId::from("checked/tutorial")),
        FaceFocus::Contribution {
            role: FaceContributionRole::Tutorial,
            node_key: Some("concept/mitochondrion".into()),
        },
        vec![contribution],
    )
    .unwrap();

    assert_eq!(face.presentation.interaction_context.identity, context_id);
    let navigation = PresentationNavigation::new(
        &face.presentation,
        vec![NavigationPlace {
            place: PresentationPlace::Program,
            root_subject: "concept/mitochondrion".into(),
            name: "Lesson".into(),
            aspects: vec![NavigationAspect {
                aspect: PresentationAspect::Structure,
                focusable_subjects: vec!["concept/mitochondrion".into()],
            }],
        }],
        vec![],
    )
    .unwrap();
    let projection = PresentationProjection::new(
        &face.presentation,
        &navigation,
        vec![
            ProjectionMembership {
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                item: ProjectionItem::Subject("concept/mitochondrion".into()),
                depth: PresentationDepth::Primary,
            },
            ProjectionMembership {
                place: PresentationPlace::Program,
                aspect: PresentationAspect::Structure,
                item: ProjectionItem::Composition(0),
                depth: PresentationDepth::Primary,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        projection
            .project(
                &face.presentation,
                &navigation,
                &PresentationCursor {
                    presentation: face.presentation.identity.clone(),
                    navigation: navigation.identity.clone(),
                    revision: face.presentation.revision,
                    place: PresentationPlace::Program,
                    aspect: PresentationAspect::Structure,
                    focus: None,
                    depth: PresentationDepth::Primary,
                },
            )
            .unwrap()
            .items
            .len(),
        2
    );
    assert!(render_linear_presentation(&face.presentation)
        .unwrap()
        .lines
        .iter()
        .any(|line| line.contains("biology/cell/organelle")));
    assert!(render_linear_presentation(&face.presentation)
        .unwrap()
        .lines
        .iter()
        .any(|line| line.contains("kind=Emphasize")));
    let generated = GenerativePresenterRequest::from_presentation(
        "request/lesson".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "mask/generative@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Use only supplied truth".into(),
        },
        face.presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    assert_eq!(generated.semantic_data.presentation, face.presentation);
}

#[test]
fn fulfilled_body_keeps_terminal_surface_without_wake_or_actions() {
    let fulfilled = born_body()
        .fulfill(
            BodyFulfillment {
                final_wake_id: None,
                authority_grant_id: AuthorityGrantId::from("grant/operator-fulfill"),
                attribution: "operator/alice".into(),
                settled_obligations: vec![FulfillmentObligation {
                    obligation_id: "obligation/closed".into(),
                    settlement_sign_id: SignId::from("sign/closed"),
                }],
            },
            SignId::from("sign/fulfilled"),
        )
        .unwrap();
    let surface = Face::project(
        &fulfilled,
        None,
        40,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();

    assert!(!surface.operator_actions.iter().any(|action| matches!(
        action.kind,
        FaceOperatorActionKind::Wake | FaceOperatorActionKind::Lull
    )));
    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "lifecycle-state"
            && property.value == PresentationPropertyValue::Text("fulfilled".into())
    }));
    assert!(surface
        .presentation
        .basis
        .sign_ids
        .iter()
        .any(|sign| sign.as_str() == "sign/fulfilled"));
}
