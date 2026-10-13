//! Source policy evaluation only; current controller/Body custody remains external.
use conduit_core::{kind_id, StructuredFieldValue, StructuredInfoType, StructuredInfoValue};
use conduit_face::{ActionInputBindingProposalInfo, GeneratedActionAffordance};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn boolean(name: &str, value: bool) -> StructuredFieldValue {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(conduit_core::BOOL_INFO_ID)).unwrap(),
            vec![u8::from(value)],
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn checked_phase_and_remap_policy_preserves_context_and_exact_action_semantics() {
    let declarations = include_str!("../types.conduit");
    let start = declarations
        .find("type GeneratedActionAffordance =")
        .unwrap();
    let end = declarations[start..].find("\n}\n").unwrap() + start + 3;
    let binding = declarations
        .find("type ActionInputBindingProposalInfo =")
        .unwrap();
    let source = format!(
        "{}\n{}\n{}\n{}",
        &declarations[start..end],
        &declarations[binding..],
        include_str!("../components/parsec-input-policy.conduit"),
        include_str!("../components/parsec-state-step.conduit")
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "parsec/input-policy",
        &ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let program = expanded.expanded.gears[0]
        .configuration
        .iter()
        .find(|field| field.key == "program")
        .unwrap();
    let conduit_core::ConfigurationValue::Text(program) = &program.value else {
        panic!("checked program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
    for started in [false, true] {
        for keyboard in [false, true] {
            for available in [false, true] {
                let context = ActionInputBindingProposalInfo::new(
                    available,
                    "boot/current".into(),
                    "control/forged".into(),
                    "Forged label".into(),
                    "face/current".into(),
                    "host/current".into(),
                    "plan/current-input".into(),
                    2,
                    GeneratedActionAffordance::new("game/forged".into(), 7).unwrap(),
                    "show/current".into(),
                    1,
                )
                .unwrap();
                let input = StructuredInfoValue::record(
                    program.input_type.clone(),
                    vec![
                        boolean("started", started),
                        boolean("keyboard", keyboard),
                        StructuredFieldValue::new(
                            "context",
                            context.clone().into_structured().unwrap(),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap();
                let output = program.evaluate(&input.canonical_bytes().unwrap()).unwrap();
                let proposal = ActionInputBindingProposalInfo::from_structured(
                    StructuredInfoValue::from_canonical_bytes(&output).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    proposal.offered_action().action_identity(),
                    if started { "game/shoot" } else { "game/start" }
                );
                assert_eq!(
                    proposal.control_identity(),
                    if keyboard {
                        "keyboard/enter"
                    } else {
                        "control/fire"
                    }
                );
                assert_eq!(
                    proposal.control_label(),
                    if keyboard { "Enter" } else { "Space" }
                );
                assert_eq!(proposal.available(), context.available());
                assert_eq!(proposal.boot_id(), context.boot_id());
                assert_eq!(proposal.host_id(), context.host_id());
                assert_eq!(proposal.input_plan_id(), context.input_plan_id());
                assert_eq!(proposal.mapping_revision(), context.mapping_revision());
                assert_eq!(proposal.face_identity(), context.face_identity());
                assert_eq!(proposal.show_identity(), context.show_identity());
                assert_eq!(
                    proposal.offered_action().source_face_revision(),
                    context.offered_action().source_face_revision()
                );
            }
        }
    }
    // Carry the actual source transition's Boolean output into the policy.
    // No Rust reducer decides that gameplay has started.
    let step =
        expand_canonical_plot_for_authoring(&checked, "parsec/state-step", &ProfileCatalog::new())
            .unwrap();
    let configuration = step.expanded.gears[0]
        .configuration
        .iter()
        .find(|field| field.key == "program")
        .unwrap();
    let conduit_core::ConfigurationValue::Text(encoded) = &configuration.value else {
        panic!("checked program")
    };
    let step = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let request = StructuredInfoValue::record(
        step.input_type.clone(),
        vec![
            boolean("started", false),
            StructuredFieldValue::new(
                "action",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id("value/text")).unwrap(),
                    b"game/start".to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let state = StructuredInfoValue::from_canonical_bytes(
        &step.evaluate(&request.canonical_bytes().unwrap()).unwrap(),
    )
    .unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = state.shape() else {
        panic!("step result")
    };
    let started = fields
        .iter()
        .find(|field| field.name() == "started")
        .unwrap();
    let context = ActionInputBindingProposalInfo::new(
        true,
        "boot/current".into(),
        "control/fire".into(),
        "Fire".into(),
        "face/gameplay".into(),
        "host/current".into(),
        "plan/current-input".into(),
        3,
        GeneratedActionAffordance::new("game/start".into(), 8).unwrap(),
        "show/gameplay".into(),
        1,
    )
    .unwrap();
    for keyboard in [false, true] {
        let request = StructuredInfoValue::record(
            program.input_type.clone(),
            vec![
                started.clone(),
                boolean("keyboard", keyboard),
                StructuredFieldValue::new("context", context.clone().into_structured().unwrap())
                    .unwrap(),
            ],
        )
        .unwrap();
        let output = program
            .evaluate(&request.canonical_bytes().unwrap())
            .unwrap();
        let proposal = ActionInputBindingProposalInfo::from_structured(
            StructuredInfoValue::from_canonical_bytes(&output).unwrap(),
        )
        .unwrap();
        assert_eq!(proposal.offered_action().action_identity(), "game/shoot");
        assert_eq!(
            proposal.control_label(),
            if keyboard { "Enter" } else { "Space" }
        );
        assert_eq!(proposal.face_identity(), "face/gameplay");
        assert_eq!(*proposal.offered_action().source_face_revision(), 8);
    }
}
