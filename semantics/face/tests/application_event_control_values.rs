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
