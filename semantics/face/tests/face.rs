use conduit_body::{Body, BodyFulfillment, FulfillmentObligation, Wake};
use conduit_core::{
    bind_active_play, kind_id, seal_plan, AuthorityGrantId, CheckedPlotId, ExpandedPlotId,
    PlotIdentity, ResourceSemanticIdentity, ResourceVersionIdentity, SignId, SourceDocumentId,
};
use conduit_face::{
    render_linear_face, BodyFace, CommittedFaceAdmission, CommittedStateBasisRefusal,
    CommittedStateContributionBasis, CommittedStateOperation, CommittedStateSelection, FaceAction,
    FaceActionAvailability, FaceAspect, FaceCompositionKind, FaceCompositionRelation, FaceContext,
    FaceContribution, FaceContributionBasis, FaceContributionRole, FaceCursor, FaceDepth,
    FaceDisclosure, FaceDisclosureLevel, FaceFocus, FaceFragment, FaceNames, FaceNavigation,
    FaceOperatorActionKind, FacePlace, FaceProjection, FacePropertyValue, FaceRefusal,
    FaceResidentPlotName, FaceRole, FaceSubject, GenerativeNarratorRole, GenerativePresenterBounds,
    GenerativePresenterPolicy, GenerativePresenterRequest, NavigationAspect, NavigationPlace,
    ProjectionItem, ProjectionMembership,
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
    let face = BodyFace::project_with_names(
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
        .face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Body)
        .unwrap();
    let plot_subject = face
        .face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Plot)
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
    assert!(face.face.properties.iter().any(|property| {
        property.subject == plot_subject.identity
            && property.name == "checked-plot-id"
            && property.value
                == FacePropertyValue::Identity(resident.checked_plot_id.as_str().into())
    }));
    assert!(face.face.text.iter().any(|text| {
        text.subject == body_subject.identity
            && text.text == "North Station is lulled with one resident plot."
    }));
    face.face.validate().unwrap();
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
        BodyFace::project_with_names(
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
) -> FaceFragment {
    FaceFragment {
        basis: FaceContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/tutorial"),
            plan_id,
            active_play_id: play_id,
            required_interaction_context: None,
        },
        subjects: vec![
            FaceSubject {
                identity: "tutorial".into(),
                role: FaceRole::Semantic(kind_id("education/tutorial")),
                name: "Learn this body".into(),
            },
            FaceSubject {
                identity: "continue".into(),
                role: FaceRole::Action,
                name: "Continue".into(),
            },
        ],
        relationships: vec![],
        composition: vec![],
        properties: vec![],
        text: vec![],
        actions: vec![FaceAction {
            identity: "tutorial.continue".into(),
            intent: "education/tutorial/continue@1".into(),
            target: "continue".into(),
            name: "Continue".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    }
}

#[test]
fn lulled_body_has_an_exact_surface_without_a_running_plot() {
    let body = born_body();
    let surface = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();

    surface.face.validate().unwrap();
    assert_eq!(surface.face.basis.body_id, Some(body.body_id));
    assert_eq!(surface.face.basis.wake_id, None);
    assert_eq!(surface.face.revision, 7);
    assert!(surface.face.properties.iter().any(|property| {
        property.name == "lifecycle-state"
            && property.value == FacePropertyValue::Text("lulled".into())
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
    let contribution = FaceContribution::from_face(
        FaceContributionRole::Tutorial,
        tutorial_fragment(plan_id.clone(), play_id.clone()),
    );
    let surface = BodyFace::project(
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

    assert!(surface.face.properties.iter().any(|property| {
        property.name == "active-play-id"
            && property.value == FacePropertyValue::Identity(play_id.as_str().into())
    }));
    assert!(surface.face.properties.iter().any(|property| {
        property.name == "plan-id"
            && property.value == FacePropertyValue::Identity(plan_id.as_str().into())
    }));
    assert!(surface
        .face
        .subjects
        .iter()
        .any(|subject| subject.role == FaceRole::Plan));
    assert!(surface
        .face
        .subjects
        .iter()
        .any(|subject| subject.role == FaceRole::Play));
    let action = surface
        .face
        .actions
        .iter()
        .find(|action| action.name == "Continue")
        .unwrap();
    assert!(surface.face.resolve_action(20, &action.identity).is_ok());
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
        BodyFace::project(
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
fn face_only_navigation_does_not_change_body_or_running_work() {
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution::from_face(
        FaceContributionRole::Foreground,
        tutorial_fragment(plan_id, play_id),
    );
    let overview = BodyFace::project(
        &body,
        Some(&wake),
        1,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![contribution.clone()],
    )
    .unwrap();
    let foreground = BodyFace::project(
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

    assert_eq!(overview.face.basis.body_id, foreground.face.basis.body_id);
    assert_eq!(overview.face.basis.wake_id, foreground.face.basis.wake_id);
    assert_eq!(body.workload_revision, 0);
    assert_eq!(wake.plans.len(), 1);
    assert_ne!(overview.face.identity, foreground.face.identity);
}

#[test]
fn composition_is_finite_and_deterministic() {
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution::from_face(
        FaceContributionRole::Tutorial,
        tutorial_fragment(plan_id, play_id),
    );
    let project = || {
        BodyFace::project(
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
    assert_eq!(first.face.identity, repeat.face.identity);
    assert_eq!(
        BodyFace::project(
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
) -> FaceFragment {
    FaceFragment {
        basis: FaceContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/tutorial"),
            plan_id,
            active_play_id: play_id,
            required_interaction_context: required_context.map(Into::into),
        },
        subjects: vec![FaceSubject {
            identity: subject_identity,
            role: FaceRole::Semantic(kind_id("education/concept")),
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
fn lulled_committed_contribution_requires_exact_selection_and_read_play() {
    let body = born_body();
    let selection = CommittedStateSelection {
        resource: ResourceSemanticIdentity::from_digest([1; 32]),
        selected_version: ResourceVersionIdentity::from_digest([2; 32]),
        published_version: ResourceVersionIdentity::from_digest([2; 32]),
    };
    let write = CommittedStateOperation {
        plan_id: "plan/todo-write".into(),
        play_id: "play/todo-write".into(),
        terminal_sign_id: "sign/todo-write".into(),
    };
    let read = CommittedStateOperation {
        plan_id: "plan/todo-read".into(),
        play_id: "play/todo-read".into(),
        terminal_sign_id: "sign/todo-read".into(),
    };
    let basis = CommittedStateContributionBasis {
        body_id: body.body_id.clone(),
        checked_plot_id: CheckedPlotId::from("checked/tutorial"),
        selection: selection.clone(),
        write,
        read: read.clone(),
        state_digest: [3; 32],
    };
    let contribution = FaceContribution::from_face(
        FaceContributionRole::Foreground,
        minimal_fragment(
            read.plan_id.clone(),
            read.play_id.clone(),
            None,
            "concept/todo-list".into(),
        ),
    );
    assert_eq!(
        BodyFace::project(
            &body,
            None,
            1,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![contribution.clone()]
        ),
        Err(FaceRefusal::PlayNotCurrent)
    );
    let face = BodyFace::project_committed(
        &body,
        1,
        FaceContext::Overview,
        FaceFocus::Body,
        contribution.clone(),
        FaceNames::default(),
        CommittedFaceAdmission {
            basis: &basis,
            current_selection: &selection,
        },
    )
    .unwrap();
    assert!(face.face.basis.sign_ids.contains(&read.terminal_sign_id));
    assert!(face
        .face
        .basis
        .sign_ids
        .contains(&basis.write.terminal_sign_id));
    let stale = CommittedStateSelection {
        selected_version: ResourceVersionIdentity::from_digest([4; 32]),
        ..selection
    };
    assert_eq!(
        BodyFace::project_committed(
            &body,
            1,
            FaceContext::Overview,
            FaceFocus::Body,
            contribution,
            FaceNames::default(),
            CommittedFaceAdmission {
                basis: &basis,
                current_selection: &stale
            },
        ),
        Err(FaceRefusal::CommittedStateBasis(
            CommittedStateBasisRefusal::CheckpointSelectionChanged,
        ))
    );
}

#[test]
fn direct_contributions_cannot_replace_face_truth_or_privately_choose_context() {
    let (body, wake, plan_id, play_id) = playing();
    let project = |fragment| {
        BodyFace::project(
            &body,
            Some(&wake),
            31,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![FaceContribution::from_face(
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
    let surface = BodyFace::project(
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
    assert!(surface.face.properties.iter().any(|property| {
        property.name == "lifecycle-state"
            && property.value == FacePropertyValue::Text("fulfilled".into())
    }));
    assert!(surface
        .face
        .basis
        .sign_ids
        .iter()
        .any(|sign| sign.as_str() == "sign/fulfilled"));
}

#[path = "face/open_domain_semantics.rs"]
mod open_domain_semantics;

#[path = "face/finite_places.rs"]
mod finite_places;
