use conduit_face::*;

fn view(component: ApplicationComponent, value: &str) -> ApplicationView {
    let mut nodes = vec![ApplicationViewNode {
        parent: None,
        component,
        key: "control".into(),
        text: "Control".into(),
        value: value.into(),
        value_capacity: 16,
        action: None,
        state: ApplicationNodeState::Ready,
    }];
    if component == ApplicationComponent::Stepper {
        for key in ["first", "second"] {
            nodes.push(ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Button,
                key: key.into(),
                text: key.into(),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            });
        }
    }
    ApplicationView {
        revision: 1,
        actions: vec![],
        nodes,
    }
}

#[test]
fn progress_decimal_syntax_matches_browser_admission() {
    for value in ["+1/2", "1/+2", "000001/2", "1/000002"] {
        assert_eq!(
            view(ApplicationComponent::Progress, value).validate(),
            Err(ApplicationViewRefusal::InvalidControlValue)
        );
    }
    let maximum = view(ApplicationComponent::Progress, "65535/65535");
    assert_eq!(
        ApplicationView::decode(&maximum.encode().unwrap()),
        Ok(maximum)
    );
}

#[test]
fn zero_current_step_refuses_even_with_decimal_padding() {
    for value in ["0/2", "00/2", "00000/2"] {
        assert_eq!(
            view(ApplicationComponent::Stepper, value).validate(),
            Err(ApplicationViewRefusal::InvalidControlValue)
        );
        assert_eq!(
            view(ApplicationComponent::Progress, value).validate(),
            Ok(())
        );
    }
    let first = view(ApplicationComponent::Stepper, "01/2");
    assert_eq!(ApplicationView::decode(&first.encode().unwrap()), Ok(first));
}
