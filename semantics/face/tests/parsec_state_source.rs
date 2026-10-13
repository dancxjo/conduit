//! Checked application transition mechanism, not installed or physical gameplay.
use conduit_core::{kind_id, StructuredFieldValue, StructuredInfoType, StructuredInfoValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn boolean(value: bool) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(conduit_core::BOOL_INFO_ID)).unwrap(),
        vec![u8::from(value)],
    )
    .unwrap()
}
fn field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).unwrap()
}

#[test]
fn checked_plot_owns_start_to_gameplay_transition_and_refuses_stale_start() {
    let syntax = parse_syntax_document(include_str!("../components/parsec-state-step.conduit"));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "parsec/state-step", &ProfileCatalog::new())
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
    // Assertions compare the evaluated source program, not a Rust game reducer.
    for (started, action, next, accepted) in [
        (false, "game/start", true, true),
        (false, "game/shoot", false, false),
        (true, "game/shoot", true, true),
        (true, "game/start", true, false),
        (false, "game/invented", false, false),
        (true, "game/invented", true, false),
    ] {
        let input = StructuredInfoValue::record(
            program.input_type.clone(),
            vec![
                field("started", boolean(started)),
                field(
                    "action",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(kind_id("value/text")).unwrap(),
                        action.as_bytes().to_vec(),
                    )
                    .unwrap(),
                ),
            ],
        )
        .unwrap();
        let actual = program.evaluate(&input.canonical_bytes().unwrap()).unwrap();
        let expected = StructuredInfoValue::record(
            program.output_type.clone(),
            vec![
                field("started", boolean(next)),
                field("accepted", boolean(accepted)),
            ],
        )
        .unwrap();
        assert_eq!(
            actual,
            expected.canonical_bytes().unwrap(),
            "{started} {action}"
        );
    }
}
