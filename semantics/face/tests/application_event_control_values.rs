use conduit_face::*;

fn node(component: ApplicationComponent, parent: Option<u8>, key: &str) -> ApplicationViewNode {
    ApplicationViewNode {
        parent,
        component,
        key: key.into(),
        text: key.into(),
        value: String::new(),
        value_capacity: 0,
        action: None,
        state: ApplicationNodeState::Ready,
    }
}

fn event(value: &[u8]) -> ApplicationEvent {
    ApplicationEvent {
        revision: 1,
        action: "control.change".into(),
        kind: ApplicationEventKind::Change,
        value: value.to_vec(),
    }
}

fn view(nodes: Vec<ApplicationViewNode>) -> ApplicationView {
    ApplicationView {
        revision: 1,
        actions: vec![ApplicationAction {
            id: "control.change".into(),
            event: ApplicationEventKind::Change,
        }],
        nodes,
    }
}

#[test]
fn native_boolean_values_round_trip_and_other_values_refuse() {
    for component in [
        ApplicationComponent::IndependentChoice,
        ApplicationComponent::ExclusiveChoice,
    ] {
        let mut control = node(component, Some(2), "control");
        control.value = "false".into();
        control.value_capacity = 5;
        control.action = Some(0);
        let view = view(vec![
            node(ApplicationComponent::ChoiceGroup, None, "group"),
            node(ApplicationComponent::ChoiceGroupLabel, Some(0), "legend"),
            node(ApplicationComponent::ChoiceOptionLabel, Some(0), "label"),
            control,
        ]);
        view.validate().unwrap();
        for value in [b"true".as_slice(), b"false"] {
            let event = event(value);
            assert_eq!(event.validate(&view), Ok(()));
            assert_eq!(
                ApplicationEvent::decode(&event.encode(&view).unwrap(), &view),
                Ok(event)
            );
        }
        for value in [b"".as_slice(), b"yes", b"TRUE", &[0xff]] {
            assert_eq!(
                event(value).validate(&view),
                Err(ApplicationViewRefusal::InvalidControlValue)
            );
        }
    }
}

#[test]
fn select_values_must_belong_to_the_bound_controls_own_options() {
    let mut select = node(ApplicationComponent::Select, None, "select");
    select.value = "one".into();
    select.value_capacity = 16;
    select.action = Some(0);
    let mut first = node(ApplicationComponent::Option, Some(0), "one");
    first.value = "one".into();
    first.value_capacity = 16;
    let mut second = node(ApplicationComponent::Option, Some(0), "unicode");
    second.value = "é😀".into();
    second.value_capacity = 16;
    let view = view(vec![select, first, second]);
    view.validate().unwrap();
    for value in ["one", "é😀"] {
        let event = event(value.as_bytes());
        assert_eq!(
            ApplicationEvent::decode(&event.encode(&view).unwrap(), &view),
            Ok(event)
        );
    }
    for value in [b"forged".as_slice(), b"", &[0xff], b"\xef\xbb\xbfone"] {
        assert_eq!(
            event(value).validate(&view),
            Err(ApplicationViewRefusal::InvalidControlValue)
        );
    }
}

#[test]
fn a_select_cannot_borrow_another_controls_option() {
    let mut selected = node(ApplicationComponent::Select, Some(0), "selected");
    selected.value = "one".into();
    selected.value_capacity = 16;
    selected.action = Some(0);
    let mut own = node(ApplicationComponent::Option, Some(1), "own");
    own.value = "one".into();
    own.value_capacity = 16;
    let mut other = node(ApplicationComponent::Select, Some(0), "other");
    other.value = "forged".into();
    other.value_capacity = 16;
    let mut foreign = node(ApplicationComponent::Option, Some(3), "foreign");
    foreign.value = "forged".into();
    foreign.value_capacity = 16;
    let view = view(vec![
        node(ApplicationComponent::Shell, None, "root"),
        selected,
        own,
        other,
        foreign,
    ]);
    view.validate().unwrap();
    assert_eq!(event(b"one").validate(&view), Ok(()));
    assert_eq!(
        event(b"forged").validate(&view),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
}

#[test]
fn graph_selection_is_an_exact_subject_of_the_current_canvas() {
    use conduit_face::application_canvas::{ApplicationCanvas, CanvasCord, CanvasNode, CanvasPort};
    let canvas = ApplicationCanvas {
        checked_plot: "checked".into(),
        expanded_plot: "expanded".into(),
        plan: "plan".into(),
        body_plan: "body-plan".into(),
        play: "play".into(),
        selected: String::new(),
        nodes: vec![CanvasNode {
            identity: "gear/é😀".into(),
            label: "Gear label".into(),
            kind: "kind".into(),
        }],
        ports: vec![
            CanvasPort {
                identity: "port/out".into(),
                owner: "gear/é😀".into(),
                label: "Output".into(),
                direction: "output".into(),
                info: "text".into(),
                temporal: "stream".into(),
            },
            CanvasPort {
                identity: "port/in".into(),
                owner: "gear/é😀".into(),
                label: "Input".into(),
                direction: "input".into(),
                info: "text".into(),
                temporal: "stream".into(),
            },
        ],
        cords: vec![CanvasCord {
            identity: "cord/one".into(),
            source: "port/out".into(),
            sink: "port/in".into(),
        }],
    };
    let mut control = node(ApplicationComponent::PatchbayCanvas, None, "canvas");
    control.value = canvas.encode().unwrap();
    control.value_capacity = 65_536;
    control.action = Some(0);
    let mut view = view(vec![control]);
    view.actions[0].event = ApplicationEventKind::Activate;
    view.validate().unwrap();
    let selection = |value: &str| ApplicationEvent {
        kind: ApplicationEventKind::Activate,
        ..event(value.as_bytes())
    };
    for identity in ["gear/é😀", "port/out", "port/in", "cord/one"] {
        let event = selection(identity);
        assert_eq!(
            ApplicationEvent::decode(&event.encode(&view).unwrap(), &view),
            Ok(event)
        );
    }
    for identity in ["", "gear/absent", "Gear label", "plan", "\u{feff}port/out"] {
        assert_eq!(
            selection(identity).validate(&view),
            Err(ApplicationViewRefusal::InvalidControlValue)
        );
    }
    let stale = ApplicationEvent {
        revision: 2,
        ..selection("port/out")
    };
    assert_eq!(
        stale.validate(&view),
        Err(ApplicationViewRefusal::StaleRevision)
    );
}

#[test]
fn initial_selection_must_be_an_owned_option_unless_it_is_empty() {
    let mut select = node(ApplicationComponent::Select, None, "select");
    select.value = "absent".into();
    select.value_capacity = 16;
    let mut option = node(ApplicationComponent::Option, Some(0), "one");
    option.value = "é😀".into();
    option.value_capacity = 16;
    let mut view = view(vec![select, option]);
    assert_eq!(
        view.validate(),
        Err(ApplicationViewRefusal::InvalidControlValue)
    );
    view.nodes[0].value = "é😀".into();
    assert_eq!(
        ApplicationView::decode(&view.encode().unwrap()),
        Ok(view.clone())
    );
    view.nodes[0].value.clear();
    assert_eq!(view.validate(), Ok(()));
    view.nodes.truncate(1);
    assert_eq!(view.validate(), Ok(()));
}
