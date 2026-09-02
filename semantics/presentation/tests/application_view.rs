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
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Heading,
                key: "heading".into(),
                text: "One bounded application view".into(),
                value: String::new(),
                value_capacity: 0,
                action: None,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Button,
                key: "next".into(),
                text: "Continue".into(),
                value: String::new(),
                value_capacity: 0,
                action: Some(0),
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
}

#[test]
fn status_outcomes_remain_renderer_neutral_and_round_trip_exactly() {
    for component in [
        ApplicationComponent::Status,
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
    wrong_version[0] = 2;
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
