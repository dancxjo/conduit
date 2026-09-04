use conduit_presentation::*;

fn view() -> ApplicationView {
    ApplicationView {
        revision: 7,
        actions: vec![ApplicationAction {
            id: "lesson.next".into(),
            event: ApplicationEventKind::Activate,
        }],
        nodes: vec![
            ApplicationViewNode {
                parent: None,
                component: ApplicationComponent::Shell,
                key: "shell".into(),
                text: String::new(),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Heading,
                key: "heading".into(),
                text: "One bounded application view".into(),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Button,
                key: "next".into(),
                text: "Continue".into(),
                value: String::new(),
                value_capacity: 0,
                action: Some(0),
                state: ApplicationNodeState::Ready,
            },
        ],
    }
}

#[test]
fn application_view_round_trip_is_exact_and_finite() {
    let original = view();
    let encoded = original.encode().unwrap();
    assert!(encoded.len() <= MAX_APPLICATION_VIEW_BYTES);
    assert_eq!(MAX_APPLICATION_VIEW_RESOURCES, 0);
    assert_eq!(ApplicationView::decode(&encoded), Ok(original));
    assert_eq!(
        CONDUIT_APPLICATION_THEME.identity,
        "conduit.presentation/phosphor@1"
    );
    assert_ne!(
        CONDUIT_APPLICATION_THEME.reading_paper,
        CONDUIT_APPLICATION_THEME.workbench_canvas
    );
    assert!(CONDUIT_APPLICATION_THEME.encode().unwrap().len() <= MAX_APPLICATION_THEME_BYTES);
    let encoded_theme = CONDUIT_APPLICATION_THEME.encode().unwrap();
    assert_eq!(encoded_theme[0], APPLICATION_THEME_VERSION);
    assert_ne!(APPLICATION_THEME_VERSION, RETIRED_APPLICATION_THEME_VERSION);
    assert_eq!(CONDUIT_APPLICATION_THEME.type_body_px, 16);
    assert_eq!(CONDUIT_APPLICATION_THEME.line_height_percent, 150);
    assert_eq!(CONDUIT_APPLICATION_THEME.space_unit_px, 4);
    assert_eq!(CONDUIT_APPLICATION_THEME.radius_panel_px, 9);
    assert_eq!(CONDUIT_APPLICATION_THEME.responsive_breakpoint_px, 720);
}

#[test]
fn status_outcomes_remain_renderer_neutral_and_round_trip_exactly() {
    for component in [
        ApplicationComponent::Status,
        ApplicationComponent::WarningStatus,
        ApplicationComponent::SuccessStatus,
        ApplicationComponent::FailureStatus,
    ] {
        let mut status = view();
        status.nodes[1].component = component;
        assert_eq!(
            ApplicationView::decode(&status.encode().unwrap()),
            Ok(status)
        );
    }
}

#[test]
fn action_availability_state_round_trips_and_refuses_inconsistent_actions() {
    for state in [
        ApplicationNodeState::Ready,
        ApplicationNodeState::Busy,
        ApplicationNodeState::Unavailable,
    ] {
        let mut candidate = view();
        candidate.nodes[2].state = state;
        if state != ApplicationNodeState::Ready {
            candidate.nodes[2].action = None;
        }
        assert_eq!(
            ApplicationView::decode(&candidate.encode().unwrap()),
            Ok(candidate)
        );
    }

    let mut inconsistent = view();
    inconsistent.nodes[2].state = ApplicationNodeState::Busy;
    assert_eq!(
        inconsistent.validate(),
        Err(ApplicationViewRefusal::InvalidNodeState)
    );
    inconsistent.nodes[2].action = None;
    inconsistent.nodes[2].component = ApplicationComponent::Panel;
    assert_eq!(
        inconsistent.validate(),
        Err(ApplicationViewRefusal::InvalidNodeState)
    );
}

#[test]
fn grouped_independent_choices_keep_semantic_identity_through_encoding() {
    let choices = ApplicationView {
        revision: 8,
        actions: vec![ApplicationAction {
            id: "form.morse.change".into(),
            event: ApplicationEventKind::Change,
        }],
        nodes: vec![
            ApplicationViewNode {
                parent: None,
                component: ApplicationComponent::ChoiceGroup,
                key: "forms".into(),
                text: "active_forms".into(),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::ChoiceGroupLabel,
                key: "forms-legend".into(),
                text: "Initial active Forms".into(),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::ChoiceOptionLabel,
                key: "morse-label".into(),
                text: "Morse Network".into(),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            },
            ApplicationViewNode {
                parent: Some(2),
                component: ApplicationComponent::IndependentChoice,
                key: "morse".into(),
                text: "morse-network".into(),
                value: "true".into(),
                value_capacity: 5,
                action: Some(0),
                state: ApplicationNodeState::Ready,
            },
        ],
    };

    assert_eq!(
        ApplicationView::decode(&choices.encode().unwrap()),
        Ok(choices.clone())
    );
    let mut malformed = choices;
    malformed.nodes[3].value = "selected".into();
    assert_eq!(
        malformed.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}

#[test]
fn navigation_links_admit_destinations_without_becoming_actions() {
    let links = ApplicationView {
        revision: 8,
        actions: vec![],
        nodes: vec![
            ApplicationViewNode {
                parent: None,
                component: ApplicationComponent::Navigation,
                key: "products".into(),
                text: "Conduit products".into(),
                value: "tour".into(),
                value_capacity: 16,
                action: None,
                state: ApplicationNodeState::Ready,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::NavigationLink,
                key: "tour".into(),
                text: "Tour".into(),
                value: "tour".into(),
                value_capacity: 16,
                action: None,
                state: ApplicationNodeState::Ready,
            },
        ],
    };
    assert_eq!(
        ApplicationView::decode(&links.encode().unwrap()),
        Ok(links.clone())
    );

    let mut arbitrary = links.clone();
    arbitrary.nodes[1].value = "https://example.invalid".into();
    arbitrary.nodes[1].value_capacity = 32;
    assert_eq!(
        arbitrary.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );

    let mut action_link = links;
    action_link.actions.push(ApplicationAction {
        id: "tour.activate".into(),
        event: ApplicationEventKind::Activate,
    });
    action_link.nodes[1].action = Some(0);
    assert_eq!(
        action_link.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}

#[test]
fn malformed_oversized_and_noncanonical_views_refuse() {
    let encoded = view().encode().unwrap();
    assert_eq!(
        ApplicationView::decode(&encoded[..encoded.len() - 1]),
        Err(ApplicationViewRefusal::MalformedEncoding)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        ApplicationView::decode(&trailing),
        Err(ApplicationViewRefusal::MalformedEncoding)
    );
    let mut wrong_version = encoded;
    wrong_version[0] = RETIRED_APPLICATION_VIEW_VERSION;
    assert_eq!(
        ApplicationView::decode(&wrong_version),
        Err(ApplicationViewRefusal::UnsupportedVersion)
    );
    assert_eq!(
        ApplicationView::decode(&vec![0; MAX_APPLICATION_VIEW_BYTES + 1]),
        Err(ApplicationViewRefusal::OversizedEncoding)
    );
}

#[test]
fn the_forty_node_product_boundary_is_exact() {
    let mut admitted = view();
    for index in admitted.nodes.len()..MAX_APPLICATION_VIEW_NODES {
        admitted.nodes.push(ApplicationViewNode {
            parent: Some(0),
            component: ApplicationComponent::Paragraph,
            key: format!("bounded-{index}"),
            text: "finite product content".into(),
            value: String::new(),
            value_capacity: 0,
            action: None,
            state: ApplicationNodeState::Ready,
        });
    }
    assert_eq!(admitted.nodes.len(), MAX_APPLICATION_VIEW_NODES);
    assert_eq!(
        ApplicationView::decode(&admitted.encode().unwrap()),
        Ok(admitted.clone())
    );

    admitted.nodes.push(ApplicationViewNode {
        parent: Some(0),
        component: ApplicationComponent::Paragraph,
        key: "one-too-many".into(),
        text: String::new(),
        value: String::new(),
        value_capacity: 0,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    assert_eq!(
        admitted.validate(),
        Err(ApplicationViewRefusal::TooManyNodes)
    );
}

#[test]
fn duplicate_keys_unknown_actions_and_depth_refuse() {
    let mut duplicate = view();
    duplicate.nodes[2].key = "heading".into();
    assert_eq!(
        duplicate.validate(),
        Err(ApplicationViewRefusal::DuplicateKey)
    );
    let mut unknown_action = view();
    unknown_action.nodes[2].action = Some(4);
    assert_eq!(
        unknown_action.validate(),
        Err(ApplicationViewRefusal::UnknownAction)
    );
    let mut deep = view();
    for index in 3..=MAX_APPLICATION_VIEW_DEPTH + 1 {
        deep.nodes.push(ApplicationViewNode {
            parent: Some((index - 1) as u8),
            component: ApplicationComponent::Stack,
            key: format!("depth-{index}"),
            text: String::new(),
            value: String::new(),
            value_capacity: 0,
            action: None,
            state: ApplicationNodeState::Ready,
        });
    }
    assert_eq!(deep.validate(), Err(ApplicationViewRefusal::TooDeep));
}

#[test]
fn events_refuse_stale_oversized_unknown_and_pressure() {
    let view = view();
    let event = ApplicationEvent {
        revision: 7,
        action: "lesson.next".into(),
        kind: ApplicationEventKind::Activate,
        value: Vec::new(),
    };
    let encoded = event.encode(&view).unwrap();
    assert_eq!(ApplicationEvent::decode(&encoded, &view), Ok(event.clone()));
    let mut queue = ApplicationEventQueue::new(1).unwrap();
    queue.push(event.clone(), &view).unwrap();
    assert_eq!(
        queue.push(event.clone(), &view),
        Err(ApplicationViewRefusal::QueuePressure)
    );
    assert_eq!(queue.pop(), Some(event));
    let stale = ApplicationEvent {
        revision: 6,
        action: "lesson.next".into(),
        kind: ApplicationEventKind::Activate,
        value: Vec::new(),
    };
    assert_eq!(
        stale.validate(&view),
        Err(ApplicationViewRefusal::StaleRevision)
    );
    let oversized = ApplicationEvent {
        revision: 7,
        action: "lesson.next".into(),
        kind: ApplicationEventKind::Activate,
        value: vec![0; MAX_APPLICATION_EVENT_BYTES + 1],
    };
    assert_eq!(
        oversized.validate(&view),
        Err(ApplicationViewRefusal::EventTooLarge)
    );
}

#[test]
fn controls_keep_labels_values_and_capacities_distinct() {
    let mut controlled = view();
    controlled.nodes.push(ApplicationViewNode {
        parent: Some(0),
        component: ApplicationComponent::TextArea,
        key: "source".into(),
        text: "Form source".into(),
        value: "gear source".into(),
        value_capacity: MAX_APPLICATION_CONTROL_VALUE_BYTES as u32,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    let encoded = controlled.encode().unwrap();
    assert_eq!(ApplicationView::decode(&encoded), Ok(controlled.clone()));

    controlled.nodes[3].value_capacity = 4;
    assert_eq!(
        controlled.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
    controlled.nodes[3].component = ApplicationComponent::Paragraph;
    assert_eq!(
        controlled.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}

#[test]
fn select_options_carry_finite_exact_values() {
    let mut selection = view();
    selection.nodes.push(ApplicationViewNode {
        parent: Some(0),
        component: ApplicationComponent::Select,
        key: "program".into(),
        text: "Initial program".into(),
        value: "morse-network@1".into(),
        value_capacity: 64,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    selection.nodes.push(ApplicationViewNode {
        parent: Some(3),
        component: ApplicationComponent::Option,
        key: "morse-program".into(),
        text: "Morse Network".into(),
        value: "morse-network@1".into(),
        value_capacity: 64,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    assert_eq!(
        ApplicationView::decode(&selection.encode().unwrap()),
        Ok(selection)
    );
}

#[test]
fn maximum_book_editor_value_round_trips_without_truncation() {
    let mut controlled = view();
    controlled.nodes.push(ApplicationViewNode {
        parent: Some(0),
        component: ApplicationComponent::TextArea,
        key: "source".into(),
        text: "Form source".into(),
        value: "x".repeat(MAX_APPLICATION_CONTROL_VALUE_BYTES),
        value_capacity: MAX_APPLICATION_CONTROL_VALUE_BYTES as u32,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    let encoded = controlled.encode().unwrap();
    assert!(encoded.len() <= MAX_APPLICATION_VIEW_BYTES);
    assert_eq!(ApplicationView::decode(&encoded), Ok(controlled));
}

#[test]
fn event_queue_refuses_aggregate_byte_pressure_and_releases_it_on_pop() {
    let mut view = view();
    view.actions[0].event = ApplicationEventKind::Input;
    view.nodes[2].component = ApplicationComponent::TextArea;
    view.nodes[2].value_capacity = 64;
    let event = ApplicationEvent {
        revision: 7,
        action: "lesson.next".into(),
        kind: ApplicationEventKind::Input,
        value: vec![0; 32],
    };
    let encoded_bytes = event.encode(&view).unwrap().len();
    let mut queue = ApplicationEventQueue::with_limits(2, encoded_bytes).unwrap();
    queue.push(event.clone(), &view).unwrap();
    assert_eq!(queue.queued_bytes(), encoded_bytes);
    assert_eq!(
        queue.push(event.clone(), &view),
        Err(ApplicationViewRefusal::QueuePressure)
    );
    assert_eq!(queue.pop(), Some(event.clone()));
    assert_eq!(queue.queued_bytes(), 0);
    queue.push(event, &view).unwrap();
}

#[test]
fn event_values_refuse_invalid_component_capacity_and_utf8() {
    let mut controlled = view();
    controlled.actions[0].event = ApplicationEventKind::Input;
    controlled.nodes[2].component = ApplicationComponent::TextArea;
    controlled.nodes[2].value_capacity = 4;
    let valid = ApplicationEvent {
        revision: 7,
        action: "lesson.next".into(),
        kind: ApplicationEventKind::Input,
        value: b"four".to_vec(),
    };
    assert_eq!(valid.validate(&controlled), Ok(()));

    let over_capacity = ApplicationEvent {
        value: b"five!".to_vec(),
        ..valid.clone()
    };
    assert_eq!(
        over_capacity.validate(&controlled),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
    let invalid_utf8 = ApplicationEvent {
        value: vec![0xff],
        ..valid
    };
    assert_eq!(
        invalid_utf8.validate(&controlled),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );

    controlled.nodes[2].component = ApplicationComponent::Button;
    controlled.nodes[2].value_capacity = 0;
    let button_value = ApplicationEvent {
        revision: 7,
        action: "lesson.next".into(),
        kind: ApplicationEventKind::Input,
        value: b"not allowed".to_vec(),
    };
    assert_eq!(
        button_value.validate(&controlled),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}
