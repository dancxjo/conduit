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
                        PresentationMechanism::ActionGroup {
                            label: "Play actions".into(),
                        },
                        vec![
                            node("run", PresentationMechanism::Action(run.clone()), vec![]),
                            node("download", PresentationMechanism::Download(run), vec![]),
                        ],
                    ),
                    node(
                        "evidence",
                        PresentationMechanism::Evidence(EvidencePresentation {
                            title: "Evidence".into(),
                            disposition: EvidenceDisposition::Succeeded,
                            identity: "evidence-7".into(),
                            provenance: "play-7".into(),
                        }),
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
        PresentationMechanismKind::ChoiceGroup,
        PresentationMechanismKind::Navigation,
        PresentationMechanismKind::NavigationLink,
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
fn choices_and_links_lower_from_meaning_without_host_manifestation_terms() {
    let change = |identity: &str, label: &str| SemanticAction {
        identity: identity.into(),
        event: ApplicationEventKind::Change,
        label: label.into(),
        availability: ActionAvailability::Available,
    };
    let lowered = SemanticApplicationView {
        revision: 8,
        root: node(
            "shell",
            PresentationMechanism::Shell,
            vec![
                node(
                    "forms",
                    PresentationMechanism::ChoiceGroup {
                        label: "Initial active forms".into(),
                        multiplicity: ChoiceMultiplicity::Independent,
                        options: vec![
                            ChoiceOption {
                                identity: "morse-network".into(),
                                label: "Morse Network".into(),
                                selected: false,
                                change_action: change("form.morse.change", "Change Morse Network"),
                            },
                            ChoiceOption {
                                identity: "memory-lantern".into(),
                                label: "Memory Lantern".into(),
                                selected: true,
                                change_action: change(
                                    "form.memory.change",
                                    "Change Memory Lantern",
                                ),
                            },
                        ],
                    },
                    vec![],
                ),
                node(
                    "products",
                    PresentationMechanism::Navigation {
                        label: "Conduit products".into(),
                        current: "tour".into(),
                    },
                    vec![node(
                        "tour",
                        PresentationMechanism::NavigationLink {
                            label: "Tour".into(),
                            destination: AdmittedNavigationDestination::Tour,
                        },
                        vec![],
                    )],
                ),
            ],
        ),
    }
    .lower()
    .unwrap();

    assert_eq!(
        lowered.nodes[1].component,
        ApplicationComponent::ChoiceGroup
    );
    assert_eq!(lowered.nodes[1].text, "forms");
    assert_eq!(
        lowered.nodes[2].component,
        ApplicationComponent::ChoiceGroupLabel
    );
    assert_eq!(
        lowered.nodes[4].component,
        ApplicationComponent::IndependentChoice
    );
    assert_eq!(lowered.nodes[4].value, "false");
    assert_eq!(lowered.nodes[6].value, "true");
    assert_eq!(lowered.nodes[7].component, ApplicationComponent::Navigation);
    assert_eq!(
        lowered.nodes[8].component,
        ApplicationComponent::NavigationLink
    );
    assert_eq!(lowered.nodes[8].value, "tour");
    assert_eq!(
        ApplicationView::decode(&lowered.encode().unwrap()),
        Ok(lowered)
    );
}

#[test]
fn exclusive_choices_refuse_multiple_selected_meanings() {
    let option = |identity: &str| ChoiceOption {
        identity: identity.into(),
        label: identity.into(),
        selected: true,
        change_action: SemanticAction {
            identity: format!("{identity}.change"),
            event: ApplicationEventKind::Change,
            label: "Change selection".into(),
            availability: ActionAvailability::Available,
        },
    };
    let result = SemanticApplicationView {
        revision: 1,
        root: node(
            "subject",
            PresentationMechanism::ChoiceGroup {
                label: "Subject".into(),
                multiplicity: ChoiceMultiplicity::Exclusive,
                options: vec![option("one"), option("two")],
            },
            vec![],
        ),
    }
    .lower();
    assert_eq!(result, Err(SemanticPresentationRefusal::InvalidChoiceGroup));
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
        help: "Choose a compatible device".into(),
        error: None,
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
fn availability_and_warning_severity_survive_semantic_lowering() {
    let unavailable = SemanticAction {
        identity: "artifact.download".into(),
        event: ApplicationEventKind::Activate,
        label: "Download".into(),
        availability: ActionAvailability::Unavailable {
            detail: "artifact absent".into(),
        },
    };
    let busy = SemanticAction {
        identity: "example.run".into(),
        event: ApplicationEventKind::Activate,
        label: "Run".into(),
        availability: ActionAvailability::Busy {
            detail: "operation admitted".into(),
        },
    };
    let lowered = SemanticApplicationView {
        revision: 2,
        root: node(
            "panel",
            PresentationMechanism::Panel {
                title: "Execution".into(),
            },
            vec![
                node("run", PresentationMechanism::Action(busy), vec![]),
                node(
                    "download",
                    PresentationMechanism::Download(unavailable),
                    vec![],
                ),
                node(
                    "warning",
                    PresentationMechanism::Status {
                        kind: StatusKind::Warning,
                        title: "Pressure".into(),
                        detail: "queue nearly full".into(),
                    },
                    vec![],
                ),
            ],
        ),
    }
    .lower()
    .unwrap();

    assert_eq!(lowered.nodes[1].state, ApplicationNodeState::Busy);
    assert_eq!(lowered.nodes[2].state, ApplicationNodeState::Unavailable);
    assert_eq!(lowered.nodes[1].action, None);
    assert_eq!(lowered.nodes[2].action, None);
    assert_eq!(
        lowered.nodes[3].component,
        ApplicationComponent::WarningStatus
    );
    assert!(lowered.actions.is_empty());
}

#[test]
fn select_options_lower_as_finite_children_of_the_exact_field() {
    let choice = FormField {
        label: "Device".into(),
        help: "Choose a compatible device".into(),
        error: Some("Previous device is stale".into()),
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
    assert_eq!(lowered.nodes[0].component, ApplicationComponent::FormField);
    assert_eq!(lowered.nodes[1].parent, Some(0));
    assert_eq!(lowered.nodes[1].component, ApplicationComponent::FieldLabel);
    assert_eq!(lowered.nodes[2].component, ApplicationComponent::Select);
    assert_eq!(lowered.nodes[3].component, ApplicationComponent::FieldHelp);
    assert_eq!(lowered.nodes[4].component, ApplicationComponent::FieldError);
    assert_eq!(lowered.nodes[5].parent, Some(2));
    assert_eq!(lowered.nodes[5].value, "Pico");
    assert_eq!(lowered.nodes[6].value, "ESP32");
}

#[test]
fn forms_navigation_stepper_and_progress_keep_exact_shared_contracts() {
    let field = FormField {
        label: "Source".into(),
        help: "Enter one bounded Form".into(),
        error: Some("Source is not checked".into()),
        value: "gear source".into(),
        value_capacity: 64,
        input_action: SemanticAction {
            identity: "source.input".into(),
            event: ApplicationEventKind::Input,
            label: "Edit source".into(),
            availability: ActionAvailability::Available,
        },
        kind: FieldKind::TextArea,
    };
    let lowered = SemanticApplicationView {
        revision: 9,
        root: node(
            "shell",
            PresentationMechanism::Shell,
            vec![
                node("source", PresentationMechanism::FormField(field), vec![]),
                node(
                    "pages",
                    PresentationMechanism::Navigation {
                        label: "Tour pages".into(),
                        current: "page-two".into(),
                    },
                    vec![
                        node(
                            "page-one",
                            PresentationMechanism::Action(action("page.one", "One")),
                            vec![],
                        ),
                        node(
                            "page-two",
                            PresentationMechanism::Action(action("page.two", "Two")),
                            vec![],
                        ),
                    ],
                ),
                node(
                    "steps",
                    PresentationMechanism::Stepper {
                        label: "Birth".into(),
                        current: 2,
                        total: 3,
                    },
                    vec![
                        node(
                            "step-one",
                            PresentationMechanism::Action(action("step.one", "Body")),
                            vec![],
                        ),
                        node(
                            "step-two",
                            PresentationMechanism::Action(action("step.two", "Host")),
                            vec![],
                        ),
                        node(
                            "step-three",
                            PresentationMechanism::Action(action("step.three", "Play")),
                            vec![],
                        ),
                    ],
                ),
                node(
                    "progress",
                    PresentationMechanism::Progress {
                        title: "Birth".into(),
                        current: 2,
                        total: 3,
                    },
                    vec![],
                ),
            ],
        ),
    }
    .lower()
    .unwrap();
    assert_eq!(lowered.nodes[1].component, ApplicationComponent::FormField);
    assert_eq!(lowered.nodes[2].component, ApplicationComponent::FieldLabel);
    assert_eq!(lowered.nodes[3].component, ApplicationComponent::TextArea);
    assert_eq!(lowered.nodes[4].component, ApplicationComponent::FieldHelp);
    assert_eq!(lowered.nodes[5].component, ApplicationComponent::FieldError);
    assert_eq!(lowered.nodes[6].component, ApplicationComponent::Navigation);
    assert_eq!(lowered.nodes[6].value, "page-two");
    assert_eq!(lowered.nodes[9].component, ApplicationComponent::Stepper);
    assert_eq!(lowered.nodes[9].value, "2/3");
    assert_eq!(lowered.nodes[13].component, ApplicationComponent::Progress);
    assert_eq!(
        ApplicationView::decode(&lowered.encode().unwrap()),
        Ok(lowered)
    );
}

#[test]
fn invalid_and_unbounded_form_choices_refuse_before_expansion() {
    let lower = |help: &str, value: &str, options: Vec<String>| {
        SemanticApplicationView {
            revision: 1,
            root: node(
                "choice",
                PresentationMechanism::FormField(FormField {
                    label: "Choice".into(),
                    help: help.into(),
                    error: None,
                    value: value.into(),
                    value_capacity: 16,
                    input_action: SemanticAction {
                        identity: "choice.change".into(),
                        event: ApplicationEventKind::Change,
                        label: "Choose".into(),
                        availability: ActionAvailability::Available,
                    },
                    kind: FieldKind::Select { options },
                }),
                vec![],
            ),
        }
        .lower()
    };
    assert_eq!(
        lower("", "one", vec!["one".into()]),
        Err(SemanticPresentationRefusal::InvalidField)
    );
    assert_eq!(
        lower("Help", "missing", vec!["one".into()]),
        Err(SemanticPresentationRefusal::InvalidField)
    );
    assert_eq!(
        lower("Help", "one", vec!["one".into(), "one".into()]),
        Err(SemanticPresentationRefusal::InvalidField)
    );
    assert_eq!(
        lower(
            "Help",
            "one",
            (0..MAX_APPLICATION_VIEW_NODES)
                .map(|index| format!("option-{index}"))
                .chain(core::iter::once("one".into()))
                .collect()
        ),
        Err(SemanticPresentationRefusal::ApplicationView(
            ApplicationViewRefusal::TooManyNodes
        ))
    );
}

#[test]
fn inherited_node_text_action_and_depth_bounds_refuse() {
    let oversized = SemanticApplicationView {
        revision: 1,
        root: node(
            "root",
            PresentationMechanism::CodeBlock {
                language: "text".into(),
                code: "x".repeat(MAX_APPLICATION_CONTROL_VALUE_BYTES + 1),
            },
            vec![],
        ),
    };
    assert_eq!(
        oversized.lower(),
        Err(SemanticPresentationRefusal::ApplicationView(
            ApplicationViewRefusal::InvalidControlValue
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

#[test]
fn evidence_artifact_definitions_and_code_keep_exact_bounded_data() {
    let dispositions = [
        EvidenceDisposition::Missing,
        EvidenceDisposition::Stale,
        EvidenceDisposition::Refused,
        EvidenceDisposition::Failed,
        EvidenceDisposition::Succeeded,
    ];
    let expected = [
        ApplicationComponent::MissingEvidence,
        ApplicationComponent::StaleEvidence,
        ApplicationComponent::RefusedEvidence,
        ApplicationComponent::FailedEvidence,
        ApplicationComponent::SuccessfulEvidence,
    ];
    for (disposition, component) in dispositions.into_iter().zip(expected) {
        let lowered = SemanticApplicationView {
            revision: 3,
            root: node(
                "evidence",
                PresentationMechanism::Evidence(EvidencePresentation {
                    title: "Play evidence".into(),
                    disposition,
                    identity: "sign-3".into(),
                    provenance: "play-2/plan-1".into(),
                }),
                vec![],
            ),
        }
        .lower()
        .unwrap();
        assert_eq!(lowered.nodes[0].component, component);
        assert_eq!(lowered.nodes[1].text, "Identity");
        assert_eq!(lowered.nodes[1].value, "sign-3");
        assert_eq!(lowered.nodes[2].text, "Provenance");
        assert_eq!(lowered.nodes[2].value, "play-2/plan-1");
        assert_eq!(
            ApplicationView::decode(&lowered.encode().unwrap()),
            Ok(lowered)
        );
    }

    let lowered = SemanticApplicationView {
        revision: 4,
        root: node(
            "artifact",
            PresentationMechanism::Artifact(ArtifactPresentation {
                title: "Plan receipt".into(),
                kind: "application/json".into(),
                detail: "bounded receipt".into(),
                identity: "sha256:abcd".into(),
                provenance: "host-1/play-2".into(),
                disposition: EvidenceDisposition::Succeeded,
            }),
            vec![node(
                "raw",
                PresentationMechanism::CodeBlock {
                    language: "json".into(),
                    code: "{\"safe\":\"<script>inert</script>\"}".into(),
                },
                vec![],
            )],
        ),
    }
    .lower()
    .unwrap();
    assert_eq!(lowered.nodes[0].component, ApplicationComponent::Artifact);
    assert_eq!(
        lowered.nodes[1].component,
        ApplicationComponent::SuccessfulEvidence
    );
    assert_eq!(lowered.nodes[6].component, ApplicationComponent::CodeBlock);
    assert!(lowered.nodes[6].value.contains("<script>"));
}

#[test]
fn incomplete_evidence_artifact_definition_and_code_refuse_before_lowering() {
    let lower = |mechanism| {
        SemanticApplicationView {
            revision: 1,
            root: node("root", mechanism, vec![]),
        }
        .lower()
    };
    assert_eq!(
        lower(PresentationMechanism::Evidence(EvidencePresentation {
            title: "Evidence".into(),
            disposition: EvidenceDisposition::Missing,
            identity: String::new(),
            provenance: "expected-play".into(),
        })),
        Err(SemanticPresentationRefusal::InvalidEvidence)
    );
    assert_eq!(
        lower(PresentationMechanism::Definition {
            term: String::new(),
            value: "value".into(),
        }),
        Err(SemanticPresentationRefusal::InvalidDefinition)
    );
    assert_eq!(
        lower(PresentationMechanism::CodeBlock {
            language: String::new(),
            code: "raw".into(),
        }),
        Err(SemanticPresentationRefusal::InvalidCodeBlock)
    );
    assert_eq!(
        lower(PresentationMechanism::Artifact(ArtifactPresentation {
            title: "Artifact".into(),
            kind: String::new(),
            detail: "detail".into(),
            identity: "id".into(),
            provenance: "source".into(),
            disposition: EvidenceDisposition::Failed,
        })),
        Err(SemanticPresentationRefusal::InvalidArtifact)
    );
}
