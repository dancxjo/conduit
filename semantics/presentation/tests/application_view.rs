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
                action: None,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Heading,
                key: "heading".into(),
                text: "One bounded application view".into(),
                action: None,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Button,
                key: "next".into(),
                text: "Continue".into(),
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
