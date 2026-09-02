use conduit_presentation::*;

fn action(id: &str, label: &str) -> SemanticAction {
    SemanticAction {
        identity: id.into(),
        event: ApplicationEventKind::Activate,
        label: label.into(),
        availability: ActionAvailability::Available,
    }
}

fn node(
    key: &str,
    mechanism: PresentationMechanism,
    children: Vec<SemanticPresentationNode>,
) -> SemanticPresentationNode {
    SemanticPresentationNode {
        key: key.into(),
        mechanism,
        children,
    }
}

#[test]
fn shared_vocabulary_lowers_deterministically_without_host_truth() {
    let run = action("example.run", "Run");
    let view = SemanticApplicationView {
        revision: 7,
        root: node(
            "shell",
            PresentationMechanism::Shell,
            vec![node(
                "workbench",
                PresentationMechanism::Workbench,
                vec![
                    node(
                        "status",
                        PresentationMechanism::Status {
                            kind: StatusKind::Success,
                            title: "Play complete".into(),
                            detail: "Exact evidence retained".into(),
                        },
                        vec![],
                    ),
                    node(
                        "actions",
                        PresentationMechanism::ActionGroup,
                        vec![
                            node("run", PresentationMechanism::Action(run.clone()), vec![]),
                            node("download", PresentationMechanism::Download(run), vec![]),
                        ],
                    ),
                    node(
                        "evidence",
                        PresentationMechanism::Evidence {
                            title: "Evidence".into(),
                        },
                        vec![node(
                            "plan",
                            PresentationMechanism::Definition {
                                term: "Plan".into(),
                                value: "P7".into(),
                            },
                            vec![],
                        )],
                    ),
                    node(
                        "progress",
                        PresentationMechanism::Progress {
                            title: "Birth".into(),
                            current: 2,
                            total: 4,
                        },
                        vec![],
                    ),
                ],
            )],
        ),
    };

    let first = view.lower().unwrap();
    let second = view.lower().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.revision, 7);
    assert_eq!(first.actions.len(), 1, "equal exact actions are shared");
    assert_eq!(
        first.nodes[2].component,
        ApplicationComponent::SuccessStatus
    );
    assert_eq!(
        first.nodes[2].text,
        "Play complete — Exact evidence retained"
    );
    assert_eq!(first.nodes[4].action, first.nodes[5].action);
    first.validate().unwrap();
}

#[test]
fn every_vocabulary_identity_is_distinct_and_versioned() {
    let kinds = [
        PresentationMechanismKind::Shell,
        PresentationMechanismKind::Workbench,
        PresentationMechanismKind::Panel,
        PresentationMechanismKind::ActionGroup,
        PresentationMechanismKind::Action,
        PresentationMechanismKind::Status,
        PresentationMechanismKind::Disclosure,
        PresentationMechanismKind::Evidence,
        PresentationMechanismKind::DefinitionTable,
        PresentationMechanismKind::Definition,
        PresentationMechanismKind::CodeBlock,
        PresentationMechanismKind::FormField,
        PresentationMechanismKind::Navigation,
        PresentationMechanismKind::Stepper,
        PresentationMechanismKind::Progress,
        PresentationMechanismKind::Artifact,
        PresentationMechanismKind::Download,
        PresentationMechanismKind::DeviceChoice,
        PresentationMechanismKind::PatchbayCanvas,
    ];
    for (index, kind) in kinds.iter().enumerate() {
        assert!(kind.identity().ends_with("@1"));
        assert!(!kinds[..index]
            .iter()
            .any(|other| other.identity() == kind.identity()));
    }
}

#[test]
fn invalid_progress_availability_and_device_choice_refuse_exactly() {
    let lower = |mechanism| {
        SemanticApplicationView {
            revision: 1,
            root: node("root", mechanism, vec![]),
        }
        .lower()
    };
    assert_eq!(
        lower(PresentationMechanism::Progress {
            title: "x".into(),
            current: 2,
            total: 1
        }),
        Err(SemanticPresentationRefusal::InvalidProgress)
    );

    let unavailable = SemanticAction {
        identity: "x".into(),
        event: ApplicationEventKind::Activate,
        label: "X".into(),
        availability: ActionAvailability::Unavailable {
            detail: String::new(),
        },
    };
    assert_eq!(
        lower(PresentationMechanism::Action(unavailable)),
        Err(SemanticPresentationRefusal::InvalidActionAvailability)
    );

    let choice = FormField {
        label: "Device".into(),
        value: String::new(),
        value_capacity: 8,
        input_action: action("device.choose", "Choose"),
        kind: FieldKind::Text,
    };
    assert_eq!(
        lower(PresentationMechanism::DeviceChoice(choice)),
        Err(SemanticPresentationRefusal::InvalidDeviceChoice)
    );
}

#[test]
fn select_options_lower_as_finite_children_of_the_exact_field() {
    let choice = FormField {
        label: "Device".into(),
        value: "Pico".into(),
        value_capacity: 16,
        input_action: SemanticAction {
            identity: "device.choose".into(),
            event: ApplicationEventKind::Change,
            label: "Choose".into(),
            availability: ActionAvailability::Available,
        },
        kind: FieldKind::Select {
            options: vec!["Pico".into(), "ESP32".into()],
        },
    };
    let lowered = SemanticApplicationView {
        revision: 1,
        root: node(
            "device",
            PresentationMechanism::DeviceChoice(choice),
            vec![],
        ),
    }
    .lower()
    .unwrap();
    assert_eq!(lowered.nodes[0].component, ApplicationComponent::Select);
    assert_eq!(lowered.nodes[1].parent, Some(0));
    assert_eq!(lowered.nodes[1].component, ApplicationComponent::Option);
    assert_eq!(lowered.nodes[1].value, "Pico");
    assert_eq!(lowered.nodes[2].value, "ESP32");
}

#[test]
fn inherited_node_text_action_and_depth_bounds_refuse() {
    let oversized = SemanticApplicationView {
        revision: 1,
        root: node(
            "root",
            PresentationMechanism::CodeBlock {
                language: "text".into(),
                code: "x".repeat(MAX_APPLICATION_VIEW_TEXT_BYTES),
            },
            vec![],
        ),
    };
    assert_eq!(
        oversized.lower(),
        Err(SemanticPresentationRefusal::ApplicationView(
            ApplicationViewRefusal::TextTooLong
        ))
    );

    let mut root = node(
        "level-8",
        PresentationMechanism::Panel { title: "8".into() },
        vec![],
    );
    for level in (0..8).rev() {
        root = node(
            &format!("level-{level}"),
            PresentationMechanism::Panel {
                title: level.to_string(),
            },
            vec![root],
        );
    }
    assert_eq!(
        SemanticApplicationView { revision: 1, root }.lower(),
        Err(SemanticPresentationRefusal::ApplicationView(
            ApplicationViewRefusal::TooDeep
        ))
    );
}
