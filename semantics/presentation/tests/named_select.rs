use conduit_presentation::*;

fn field() -> FormField {
    FormField {
        label: "Tradition".into(),
        help: "Choose a naming tradition.".into(),
        error: None,
        value: "tradition/a".into(),
        value_capacity: 64,
        input_action: SemanticAction {
            identity: "tradition.choose".into(),
            label: "Choose".into(),
            event: ApplicationEventKind::Change,
            availability: ActionAvailability::Available,
        },
        kind: FieldKind::NamedSelect {
            options: vec![
                SelectOption {
                    identity: "tradition/a".into(),
                    label: "Same visible label".into(),
                },
                SelectOption {
                    identity: "tradition/b".into(),
                    label: "Same visible label".into(),
                },
            ],
        },
    }
}
fn view(field: FormField) -> SemanticApplicationView {
    SemanticApplicationView {
        revision: 9,
        root: SemanticPresentationNode {
            key: "tradition".into(),
            mechanism: PresentationMechanism::FormField(field),
            children: vec![],
        },
    }
}
#[test]
fn named_select_preserves_exact_values_independent_of_labels_through_wire_encoding() {
    let lowered = view(field()).lower().unwrap();
    let decoded = ApplicationView::decode(&lowered.encode().unwrap()).unwrap();
    assert_eq!(lowered, decoded);
    let options: Vec<_> = decoded
        .nodes
        .iter()
        .filter(|node| node.component == ApplicationComponent::Option)
        .collect();
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].text, options[1].text);
    assert_eq!(options[0].value, "tradition/a");
    assert_eq!(options[1].value, "tradition/b");
    assert_eq!(decoded.actions[0].event, ApplicationEventKind::Change);
}
#[test]
fn named_select_refuses_duplicate_identity_empty_label_missing_selection_and_wrong_event() {
    for case in 0..4 {
        let mut candidate = field();
        let FieldKind::NamedSelect { options } = &mut candidate.kind else {
            unreachable!()
        };
        match case {
            0 => options[1].identity = options[0].identity.clone(),
            1 => options[0].label.clear(),
            2 => candidate.value = "Same visible label".into(),
            _ => candidate.input_action.event = ApplicationEventKind::Input,
        }
        assert_eq!(
            view(candidate).lower(),
            Err(SemanticPresentationRefusal::InvalidField)
        );
    }
}
