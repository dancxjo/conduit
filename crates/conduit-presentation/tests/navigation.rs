use conduit_presentation::{
    NavigationAspect, NavigationFollow, NavigationOperation, NavigationPlace, NavigationRefusal,
    NavigationState, Presentation, PresentationAspect, PresentationBasis, PresentationCursor,
    PresentationDepth, PresentationNavigation, PresentationPlace, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, MAX_NAVIGATION_HISTORY,
};

fn subject(identity: &str, role: PresentationRole) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role,
        label: identity.into(),
        accessibility_name: format!("{identity} subject"),
    }
}

fn presentation(revision: u64) -> Presentation {
    Presentation::new(
        revision,
        PresentationBasis {
            seed_id: None,
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            subject("entrance", PresentationRole::Document),
            subject("program", PresentationRole::Form),
            subject("cord/foo", PresentationRole::Cord),
            subject("body", PresentationRole::Body),
            subject("line/wifi-17", PresentationRole::Line),
            subject("sign/terminal", PresentationRole::Sign),
        ],
        vec![PresentationRelationship {
            source: "cord/foo".into(),
            target: "line/wifi-17".into(),
            kind: PresentationRelationshipKind::Realizes,
        }],
        vec![],
        vec![],
    )
    .unwrap()
}

fn navigation(presentation: &Presentation) -> PresentationNavigation {
    PresentationNavigation::new(
        presentation,
        vec![
            NavigationPlace {
                place: PresentationPlace::Entrance,
                root_subject: "entrance".into(),
                label: "Entrance".into(),
                aspects: vec![NavigationAspect {
                    aspect: PresentationAspect::Structure,
                    focusable_subjects: vec!["entrance".into()],
                }],
            },
            NavigationPlace {
                place: PresentationPlace::Program,
                root_subject: "program".into(),
                label: "Program".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: PresentationAspect::Structure,
                        focusable_subjects: vec!["program".into(), "cord/foo".into()],
                    },
                    NavigationAspect {
                        aspect: PresentationAspect::Plan,
                        focusable_subjects: vec!["program".into(), "cord/foo".into()],
                    },
                    NavigationAspect {
                        aspect: PresentationAspect::Signs,
                        focusable_subjects: vec!["sign/terminal".into()],
                    },
                ],
            },
            NavigationPlace {
                place: PresentationPlace::Body,
                root_subject: "body".into(),
                label: "Body".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: PresentationAspect::Structure,
                        focusable_subjects: vec!["body".into(), "line/wifi-17".into()],
                    },
                    NavigationAspect {
                        aspect: PresentationAspect::Plan,
                        focusable_subjects: vec!["body".into(), "line/wifi-17".into()],
                    },
                ],
            },
        ],
        vec![NavigationFollow {
            identity: "realized-by/wifi-17".into(),
            source_subject: "cord/foo".into(),
            relationship: PresentationRelationshipKind::Realizes,
            target_subject: "line/wifi-17".into(),
            target_place: PresentationPlace::Body,
            target_aspect: PresentationAspect::Plan,
        }],
    )
    .unwrap()
}

fn cursor(presentation: &Presentation, navigation: &PresentationNavigation) -> PresentationCursor {
    PresentationCursor {
        presentation: presentation.identity.clone(),
        navigation: navigation.identity.clone(),
        revision: presentation.revision,
        place: PresentationPlace::Entrance,
        aspect: PresentationAspect::Structure,
        focus: None,
        depth: PresentationDepth::Primary,
    }
}

#[test]
fn finite_navigation_composes_place_aspect_focus_depth_and_back() {
    let presentation = presentation(7);
    let navigation = navigation(&presentation);
    let mut state =
        NavigationState::new(&navigation, cursor(&presentation, &navigation), 8).unwrap();

    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Enter(PresentationPlace::Program),
        )
        .unwrap();
    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Show(PresentationAspect::Plan),
        )
        .unwrap();
    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Focus("cord/foo".into()),
        )
        .unwrap();
    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Disclose(PresentationDepth::Exact),
        )
        .unwrap();
    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Follow("realized-by/wifi-17".into()),
        )
        .unwrap();

    assert_eq!(state.cursor().place, PresentationPlace::Body);
    assert_eq!(state.cursor().aspect, PresentationAspect::Plan);
    assert_eq!(state.cursor().focus.as_deref(), Some("line/wifi-17"));
    assert_eq!(state.cursor().depth, PresentationDepth::Exact);
    assert_eq!(state.history_len(), 5);

    state
        .navigate(&presentation, &navigation, 7, NavigationOperation::Back)
        .unwrap();
    assert_eq!(state.cursor().place, PresentationPlace::Program);
    assert_eq!(state.cursor().focus.as_deref(), Some("cord/foo"));
}

#[test]
fn navigation_cannot_change_presentation_or_semantic_identity_basis() {
    let presentation = presentation(11);
    let before = presentation.clone();
    let navigation = navigation(&presentation);
    let mut state =
        NavigationState::new(&navigation, cursor(&presentation, &navigation), 4).unwrap();

    for operation in [
        NavigationOperation::Enter(PresentationPlace::Program),
        NavigationOperation::Show(PresentationAspect::Plan),
        NavigationOperation::Focus("cord/foo".into()),
        NavigationOperation::Follow("realized-by/wifi-17".into()),
    ] {
        state
            .navigate(&presentation, &navigation, 11, operation)
            .unwrap();
    }

    assert_eq!(presentation, before);
    assert_eq!(presentation.identity, before.identity);
    assert_eq!(presentation.basis, before.basis);
    assert_eq!(presentation.subjects, before.subjects);
    assert_eq!(presentation.relationships, before.relationships);
    assert_eq!(presentation.actions, before.actions);
}

#[test]
fn stale_unknown_and_exhausted_navigation_fail_closed_without_motion() {
    let presentation = presentation(7);
    let navigation = navigation(&presentation);
    let mut state =
        NavigationState::new(&navigation, cursor(&presentation, &navigation), 2).unwrap();
    let initial = state.cursor().clone();

    assert_eq!(
        state.navigate(
            &presentation,
            &navigation,
            6,
            NavigationOperation::Enter(PresentationPlace::Program),
        ),
        Err(NavigationRefusal::StalePresentation)
    );
    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Enter(PresentationPlace::Body),
        )
        .unwrap();
    assert_eq!(
        state.navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Show(PresentationAspect::Play),
        ),
        Err(NavigationRefusal::UnknownAspect)
    );
    assert_eq!(
        state.navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Focus("absent".into()),
        ),
        Err(NavigationRefusal::UnknownSubject)
    );
    assert_eq!(state.history_len(), 1);
    state
        .navigate(&presentation, &navigation, 7, NavigationOperation::Back)
        .unwrap();
    assert_eq!(state.cursor(), &initial);
    assert_eq!(
        state.navigate(&presentation, &navigation, 7, NavigationOperation::Back),
        Err(NavigationRefusal::HistoryExhausted)
    );
}

#[test]
fn follow_requires_exact_advertised_current_correlation() {
    let presentation = presentation(7);
    let navigation = navigation(&presentation);
    let mut state =
        NavigationState::new(&navigation, cursor(&presentation, &navigation), 4).unwrap();
    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Enter(PresentationPlace::Program),
        )
        .unwrap();
    state
        .navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Focus("cord/foo".into()),
        )
        .unwrap();
    let before = state.cursor().clone();
    assert_eq!(
        state.navigate(
            &presentation,
            &navigation,
            7,
            NavigationOperation::Follow("looks-related".into()),
        ),
        Err(NavigationRefusal::UnknownRelationship)
    );
    assert_eq!(state.cursor(), &before);
}

#[test]
fn superseded_presentation_and_bounded_history_are_refused() {
    let current = presentation(7);
    let navigation = navigation(&current);
    let superseding = presentation(8);
    let mut state = NavigationState::new(&navigation, cursor(&current, &navigation), 1).unwrap();

    assert_eq!(
        state.navigate(
            &superseding,
            &navigation,
            7,
            NavigationOperation::Enter(PresentationPlace::Program),
        ),
        Err(NavigationRefusal::StalePresentation)
    );
    state
        .navigate(
            &current,
            &navigation,
            7,
            NavigationOperation::Enter(PresentationPlace::Program),
        )
        .unwrap();
    assert_eq!(
        state.navigate(
            &current,
            &navigation,
            7,
            NavigationOperation::Show(PresentationAspect::Plan),
        ),
        Err(NavigationRefusal::HistoryFull)
    );
    assert_eq!(
        NavigationState::new(
            &navigation,
            cursor(&current, &navigation),
            MAX_NAVIGATION_HISTORY + 1,
        ),
        Err(NavigationRefusal::InvalidTruth)
    );
}

#[test]
fn changed_navigation_truth_has_a_new_identity_and_stales_the_old_cursor() {
    let presentation = presentation(7);
    let navigation = navigation(&presentation);
    let old_cursor = cursor(&presentation, &navigation);
    let mut changed_places = navigation.places.clone();
    changed_places[1].label = "Current Program".into();
    let changed =
        PresentationNavigation::new(&presentation, changed_places, navigation.follows.clone())
            .unwrap();

    assert_ne!(navigation.identity, changed.identity);
    assert_eq!(
        NavigationState::new(&changed, old_cursor, 4),
        Err(NavigationRefusal::StalePresentation)
    );
}

#[test]
fn invalid_roots_aspects_subjects_and_synthetic_follows_are_not_navigation_truth() {
    let presentation = presentation(7);
    let mut places = navigation(&presentation).places;
    places[0].root_subject = "absent".into();
    assert_eq!(
        PresentationNavigation::new(&presentation, places, vec![]),
        Err(NavigationRefusal::InvalidTruth)
    );

    let mut invalid = navigation(&presentation);
    invalid.follows[0].relationship = PresentationRelationshipKind::Observes;
    assert_eq!(
        invalid.validate(&presentation),
        Err(NavigationRefusal::InvalidTruth)
    );
}
