//! Checked source declaration/expansion only; no physical binding admission.
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{ActionInputBindingProposalInfo, GeneratedActionAffordance};

#[test]
fn source_owned_start_remap_and_gameplay_policies_have_typed_fores() {
    let declarations = include_str!("../types.conduit");
    let action_start = declarations
        .find("type GeneratedActionAffordance =")
        .unwrap();
    let action_end = declarations[action_start..].find("\n}\n").unwrap() + action_start + 3;
    let binding_start = declarations
        .find("type ActionInputBindingProposalInfo =")
        .unwrap();
    let source = format!(
        "{}\n{}\n{}",
        &declarations[action_start..action_end],
        &declarations[binding_start..],
        include_str!("../components/start-input-binding.conduit")
    );
    let startup = StartupCatalog::new();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    assert_eq!(checked.plots.len(), 3);
    let profiles = ProfileCatalog::new();
    for name in [
        "pregame-fire-binding",
        "pregame-enter-binding",
        "gameplay-fire-binding",
    ] {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
        assert_eq!(expanded.input_bindings.len(), 1);
        assert_eq!(expanded.output_bindings.len(), 1);
        let context = ActionInputBindingProposalInfo::new(
            true,
            "boot/current".into(),
            "control/caller-forged".into(),
            "Caller label".into(),
            "face/current".into(),
            "host/current".into(),
            "plan/current-input".into(),
            2,
            GeneratedActionAffordance::new("game/caller-forged".into(), 7).unwrap(),
            "show/current".into(),
            1,
        )
        .unwrap();
        let gear = &expanded.expanded.gears[0];
        let program = gear
            .configuration
            .iter()
            .find(|field| field.key == "program")
            .unwrap();
        let conduit_core::ConfigurationValue::Text(program) = &program.value else {
            panic!("checked program");
        };
        let input = context
            .clone()
            .into_structured()
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let output = conduit_plot::PortableExpressionProgram::from_canonical_hex(program)
            .unwrap()
            .evaluate(&input)
            .unwrap();
        let output = conduit_core::StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        let proposal = ActionInputBindingProposalInfo::from_structured(output).unwrap();
        let (control, label, action) = match name {
            "pregame-fire-binding" => ("control/fire", "Fire", "game/start"),
            "pregame-enter-binding" => ("keyboard/enter", "Enter", "game/start"),
            _ => ("control/fire", "Fire", "game/shoot"),
        };
        assert_eq!(proposal.control_identity(), control);
        assert_eq!(proposal.control_label(), label);
        assert_eq!(proposal.offered_action().action_identity(), action);
        assert_eq!(proposal.face_identity(), context.face_identity());
        assert_eq!(proposal.show_identity(), context.show_identity());
        assert_eq!(proposal.mapping_revision(), context.mapping_revision());
    }
}

#[test]
fn imported_shape_without_native_laws_cannot_construct_binding_proposals() {
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type(
            "ActionInputBindingProposalInfo",
            ActionInputBindingProposalInfo::semantic_type().unwrap(),
        )
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../components/start-input-binding.conduit")),
        &startup,
    )
    .unwrap();
    let refused = expand_canonical_plot_for_authoring(
        &checked,
        "pregame-fire-binding",
        &ProfileCatalog::new(),
    )
    .unwrap_err();
    assert_eq!(refused.code, "CND-FRM-046");
}
