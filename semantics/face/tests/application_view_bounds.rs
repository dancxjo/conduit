use conduit_face::*;

fn view() -> ApplicationView {
    let mut nodes = vec![ApplicationViewNode {
        parent: None,
        component: ApplicationComponent::Shell,
        key: "root".into(),
        text: String::new(),
        value: String::new(),
        value_capacity: 0,
        action: None,
        state: ApplicationNodeState::Ready,
    }];
    for key in ["a", "b"] {
        nodes.push(ApplicationViewNode {
            parent: Some(0),
            component: ApplicationComponent::TextArea,
            key: key.into(),
            text: key.into(),
            value: String::new(),
            value_capacity: 65_536,
            action: None,
            state: ApplicationNodeState::Ready,
        });
    }
    ApplicationView {
        revision: 1,
        actions: vec![],
        nodes,
    }
}

#[test]
fn view_validation_enforces_the_encoded_envelope_before_publication() {
    let mut view = view();
    view.nodes[1].value = "x".repeat(65_536);
    view.nodes[2].value = "y".repeat(65_536);
    assert_eq!(
        view.validate(),
        Err(ApplicationViewRefusal::OversizedEncoding)
    );
    assert_eq!(
        view.encode(),
        Err(ApplicationViewRefusal::OversizedEncoding)
    );
}

#[test]
fn exact_view_envelope_round_trips_and_one_more_byte_refuses() {
    let mut view = view();
    let overhead = view.encode().unwrap().len();
    view.nodes[1].value = "x".repeat(65_536);
    view.nodes[2].value = "y".repeat(MAX_APPLICATION_VIEW_BYTES - overhead - 65_536);
    assert_eq!(view.validate(), Ok(()));
    let encoded = view.encode().unwrap();
    assert_eq!(encoded.len(), MAX_APPLICATION_VIEW_BYTES);
    assert_eq!(ApplicationView::decode(&encoded), Ok(view.clone()));
    view.nodes[2].value.push('y');
    assert_eq!(
        view.validate(),
        Err(ApplicationViewRefusal::OversizedEncoding)
    );
}
