use conduit_face::{
    Face, FaceAspect, FaceBasis, FaceCursor, FaceDepth, FaceNavigation, FacePlace,
    FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject, NavigationAspect,
    NavigationFollow, NavigationOperation, NavigationPlace, NavigationRefusal, NavigationState,
    MAX_NAVIGATION_HISTORY,
};

fn subject(identity: &str, role: FaceRole) -> FaceSubject {
    FaceSubject {
        identity: identity.into(),
        role,
        name: format!("{identity} subject"),
    }
}

fn face(revision: u64) -> Face {
    Face::new(
        revision,
        FaceBasis {
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
            subject("entrance", FaceRole::Document),
            subject("program", FaceRole::Plot),
            subject("cord/foo", FaceRole::Cord),
            subject("body", FaceRole::Body),
            subject("line/wifi-17", FaceRole::Line),
            subject("sign/terminal", FaceRole::Sign),
        ],
        vec![FaceRelationship {
            source: "cord/foo".into(),
            target: "line/wifi-17".into(),
            kind: FaceRelationshipKind::Realizes,
        }],
        vec![],
        vec![],
    )
    .unwrap()
}

fn navigation(face: &Face) -> FaceNavigation {
    FaceNavigation::new(
        face,
        vec![
            NavigationPlace {
                place: FacePlace::Entrance,
                root_subject: "entrance".into(),
                name: "Entrance".into(),
                aspects: vec![NavigationAspect {
                    aspect: FaceAspect::Structure,
                    focusable_subjects: vec!["entrance".into()],
                }],
            },
            NavigationPlace {
                place: FacePlace::Program,
                root_subject: "program".into(),
                name: "Program".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: FaceAspect::Structure,
                        focusable_subjects: vec!["program".into(), "cord/foo".into()],
                    },
                    NavigationAspect {
                        aspect: FaceAspect::Plan,
                        focusable_subjects: vec!["program".into(), "cord/foo".into()],
                    },
                    NavigationAspect {
                        aspect: FaceAspect::Signs,
                        focusable_subjects: vec!["sign/terminal".into()],
                    },
                ],
            },
            NavigationPlace {
                place: FacePlace::Body,
                root_subject: "body".into(),
                name: "Body".into(),
                aspects: vec![
                    NavigationAspect {
                        aspect: FaceAspect::Structure,
                        focusable_subjects: vec!["body".into(), "line/wifi-17".into()],
                    },
                    NavigationAspect {
                        aspect: FaceAspect::Plan,
                        focusable_subjects: vec!["body".into(), "line/wifi-17".into()],
                    },
                ],
            },
        ],
        vec![NavigationFollow {
            identity: "realized-by/wifi-17".into(),
            source_subject: "cord/foo".into(),
            relationship: FaceRelationshipKind::Realizes,
            target_subject: "line/wifi-17".into(),
            target_place: FacePlace::Body,
            target_aspect: FaceAspect::Plan,
        }],
    )
    .unwrap()
}

fn cursor(face: &Face, navigation: &FaceNavigation) -> FaceCursor {
    FaceCursor {
        face: face.identity.clone(),
        navigation: navigation.identity.clone(),
        revision: face.revision,
        place: FacePlace::Entrance,
        aspect: FaceAspect::Structure,
        focus: None,
        depth: FaceDepth::Primary,
    }
}

#[test]
fn finite_navigation_composes_place_aspect_focus_depth_and_back() {
    let face = face(7);
    let navigation = navigation(&face);
    let mut state = NavigationState::new(&navigation, cursor(&face, &navigation), 8).unwrap();

    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Enter(FacePlace::Program),
        )
        .unwrap();
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Show(FaceAspect::Plan),
        )
        .unwrap();
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Focus("cord/foo".into()),
        )
        .unwrap();
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Disclose(FaceDepth::Exact),
        )
        .unwrap();
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Follow("realized-by/wifi-17".into()),
        )
        .unwrap();

    assert_eq!(state.cursor().place, FacePlace::Body);
    assert_eq!(state.cursor().aspect, FaceAspect::Plan);
    assert_eq!(state.cursor().focus.as_deref(), Some("line/wifi-17"));
    assert_eq!(state.cursor().depth, FaceDepth::Exact);
    assert_eq!(state.history_len(), 5);

    state
        .navigate(&face, &navigation, 7, NavigationOperation::Back)
        .unwrap();
    assert_eq!(state.cursor().place, FacePlace::Program);
    assert_eq!(state.cursor().focus.as_deref(), Some("cord/foo"));
}

#[test]
fn focus_and_disclose_moves_one_bounded_cursor_revision() {
    let face = face(7);
    let navigation = navigation(&face);
    let mut state = NavigationState::new(&navigation, cursor(&face, &navigation), 4).unwrap();

    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Enter(FacePlace::Program),
        )
        .unwrap();
    let before = state.cursor().clone();
    let history_before = state.history_len();
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::FocusAndDisclose("cord/foo".into(), FaceDepth::Detail),
        )
        .unwrap();

    assert_eq!(state.cursor().focus.as_deref(), Some("cord/foo"));
    assert_eq!(state.cursor().depth, FaceDepth::Detail);
    assert_eq!(state.history_len(), history_before + 1);
    state
        .navigate(&face, &navigation, 7, NavigationOperation::Back)
        .unwrap();
    assert_eq!(state.cursor(), &before);
}

#[test]
fn navigation_cannot_change_face_or_semantic_identity_basis() {
    let face = face(11);
    let before = face.clone();
    let navigation = navigation(&face);
    let mut state = NavigationState::new(&navigation, cursor(&face, &navigation), 4).unwrap();

    for operation in [
        NavigationOperation::Enter(FacePlace::Program),
        NavigationOperation::Show(FaceAspect::Plan),
        NavigationOperation::Focus("cord/foo".into()),
        NavigationOperation::Follow("realized-by/wifi-17".into()),
    ] {
        state.navigate(&face, &navigation, 11, operation).unwrap();
    }

    assert_eq!(face, before);
    assert_eq!(face.identity, before.identity);
    assert_eq!(face.basis, before.basis);
    assert_eq!(face.subjects, before.subjects);
    assert_eq!(face.relationships, before.relationships);
    assert_eq!(face.actions, before.actions);
}

#[test]
fn stale_unknown_and_exhausted_navigation_fail_closed_without_motion() {
    let face = face(7);
    let navigation = navigation(&face);
    let mut state = NavigationState::new(&navigation, cursor(&face, &navigation), 2).unwrap();
    let initial = state.cursor().clone();

    assert_eq!(
        state.navigate(
            &face,
            &navigation,
            6,
            NavigationOperation::Enter(FacePlace::Program),
        ),
        Err(NavigationRefusal::StaleFace)
    );
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Enter(FacePlace::Body),
        )
        .unwrap();
    assert_eq!(
        state.navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Show(FaceAspect::Play),
        ),
        Err(NavigationRefusal::UnknownAspect)
    );
    assert_eq!(
        state.navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Focus("absent".into()),
        ),
        Err(NavigationRefusal::UnknownSubject)
    );
    assert_eq!(state.history_len(), 1);
    state
        .navigate(&face, &navigation, 7, NavigationOperation::Back)
        .unwrap();
    assert_eq!(state.cursor(), &initial);
    assert_eq!(
        state.navigate(&face, &navigation, 7, NavigationOperation::Back),
        Err(NavigationRefusal::HistoryExhausted)
    );
}

#[test]
fn follow_requires_exact_advertised_current_correlation() {
    let face = face(7);
    let navigation = navigation(&face);
    let mut state = NavigationState::new(&navigation, cursor(&face, &navigation), 4).unwrap();
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Enter(FacePlace::Program),
        )
        .unwrap();
    state
        .navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Focus("cord/foo".into()),
        )
        .unwrap();
    let before = state.cursor().clone();
    assert_eq!(
        state.navigate(
            &face,
            &navigation,
            7,
            NavigationOperation::Follow("looks-related".into()),
        ),
        Err(NavigationRefusal::UnknownRelationship)
    );
    assert_eq!(state.cursor(), &before);
}

#[test]
fn superseded_face_and_bounded_history_are_refused() {
    let current = face(7);
    let navigation = navigation(&current);
    let superseding = face(8);
    let mut state = NavigationState::new(&navigation, cursor(&current, &navigation), 1).unwrap();

    assert_eq!(
        state.navigate(
            &superseding,
            &navigation,
            7,
            NavigationOperation::Enter(FacePlace::Program),
        ),
        Err(NavigationRefusal::StaleFace)
    );
    state
        .navigate(
            &current,
            &navigation,
            7,
            NavigationOperation::Enter(FacePlace::Program),
        )
        .unwrap();
    assert_eq!(
        state.navigate(
            &current,
            &navigation,
            7,
            NavigationOperation::Show(FaceAspect::Plan),
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
    let face = face(7);
    let navigation = navigation(&face);
    let old_cursor = cursor(&face, &navigation);
    let mut changed_places = navigation.places.clone();
    changed_places[1].name = "Current Program".into();
    let changed = FaceNavigation::new(&face, changed_places, navigation.follows.clone()).unwrap();

    assert_ne!(navigation.identity, changed.identity);
    assert_eq!(
        NavigationState::new(&changed, old_cursor, 4),
        Err(NavigationRefusal::StaleFace)
    );
}

#[test]
fn invalid_roots_aspects_subjects_and_synthetic_follows_are_not_navigation_truth() {
    let face = face(7);
    let mut places = navigation(&face).places;
    places[0].root_subject = "absent".into();
    assert_eq!(
        FaceNavigation::new(&face, places, vec![]),
        Err(NavigationRefusal::InvalidTruth)
    );

    let mut invalid = navigation(&face);
    invalid.follows[0].relationship = FaceRelationshipKind::Observes;
    assert_eq!(
        invalid.validate(&face),
        Err(NavigationRefusal::InvalidTruth)
    );
}
