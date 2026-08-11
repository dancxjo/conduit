use conduit_core::ConfigurationValue;

use crate::{FaceControlKind, FormEditor, FormEditorError, PatchbayGraph};

fn editor(source: &str) -> FormEditor {
    FormEditor::from_source("face-controls.conduit".into(), source.into()).unwrap()
}

#[test]
fn face_controls_project_actual_values_and_visible_contracts() {
    let editor = editor("form controls {\n    clock: time/every(freq = 25ms)\n}\n");
    let graph = PatchbayGraph::from_expanded(&editor.expand_form("controls").unwrap()).unwrap();
    let controls = &graph.gears[0].controls;
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].value, ConfigurationValue::U64(25));
    assert!(matches!(
        controls[0].kind,
        FaceControlKind::Number {
            unit: Some("ms"),
            ..
        }
    ));
}

#[test]
fn timing_controls_expose_bounded_duration_policy_and_capacity() {
    let editor = editor(
        "form controls {\n    stable: time/debounce(duration-ms = 25ms, policy = \"trailing\", maximum-values = 4)\n}\n",
    );
    let graph = PatchbayGraph::from_expanded(&editor.expand_form("controls").unwrap()).unwrap();
    let controls = &graph.gears[0].controls;
    assert_eq!(controls.len(), 3);
    assert!(matches!(
        controls[0].kind,
        FaceControlKind::Number {
            minimum: 0,
            maximum: conduit_std_catalog::TIME_MAXIMUM_DURATION_MS,
            unit: Some("ms")
        }
    ));
    assert!(matches!(
        controls[1].kind,
        FaceControlKind::TextChoice { ref choices }
            if choices == &[conduit_std_catalog::TIME_POLICY_TRAILING.to_string()]
    ));
    assert!(matches!(
        controls[2].kind,
        FaceControlKind::Range {
            minimum: 1,
            maximum: conduit_std_catalog::TIME_MAXIMUM_VALUES,
            unit: None
        }
    ));
}

#[test]
fn signed_scalar_control_is_visible_bounded_and_authored_exactly() {
    let mut editor =
        editor("form controls {\n    clamp: math/clamp(minimum = -7, maximum = 9)\n}\n");
    let before = editor.expand_form("controls").unwrap();
    let graph = PatchbayGraph::from_expanded(&before).unwrap();
    assert!(matches!(
        graph.gears[0].controls[0].kind,
        FaceControlKind::ScalarNumber {
            minimum: i64::MIN,
            maximum: i64::MAX,
            unit: "µ"
        }
    ));
    editor
        .set_gear_configuration(
            0,
            &before.expanded_form_id,
            "clamp",
            "minimum",
            ConfigurationValue::I64(-8),
        )
        .unwrap();
    assert!(editor.view().source.contains("minimum = -8"));
}

#[test]
fn boolean_contract_projects_a_toggle_with_explicit_choices() {
    let gear = conduit_form::CheckedGear {
        gear_id: conduit_core::GearId::from("controls/pulse"),
        kind_id: conduit_core::kind_id("flow/pulse"),
        kind_contract_revision: conduit_core::KindContractRevision::from("legacy-test"),
        startup_parameters: Vec::new(),
        shorthand: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        configuration: vec![
            conduit_core::ConfigurationEntry {
                key: "count".into(),
                value: ConfigurationValue::U64(4),
            },
            conduit_core::ConfigurationEntry {
                key: "period-ms".into(),
                value: ConfigurationValue::U64(25),
            },
            conduit_core::ConfigurationEntry {
                key: "initial".into(),
                value: ConfigurationValue::Bool(true),
            },
        ],
        pool_references: Vec::new(),
    };
    let controls = crate::face_controls::project_controls(&gear).unwrap();
    assert!(matches!(
        controls[2].kind,
        FaceControlKind::BooleanChoice {
            choices: ["false", "true"]
        }
    ));
    assert_eq!(controls[2].value, ConfigurationValue::Bool(true));
}

#[test]
fn face_edit_preserves_gear_identity_and_reseals_all_form_identities() {
    let mut editor = editor("form controls {\n    clock: time/every(freq = 25ms)\n}\n");
    let before = editor.expand_form("controls").unwrap();
    let revision = editor.view().revision;
    editor
        .set_gear_configuration(
            revision,
            &before.expanded_form_id,
            "clock",
            "freq",
            ConfigurationValue::U64(26),
        )
        .unwrap();
    let after = editor.expand_form("controls").unwrap();
    assert_eq!(before.gears[0].gear_id, after.gears[0].gear_id);
    assert_ne!(before.source_document_id, after.source_document_id);
    assert_ne!(before.checked_form_id, after.checked_form_id);
    assert_ne!(before.expanded_form_id, after.expanded_form_id);
    assert!(editor.view().source.contains("freq = 26ms"));
}

#[test]
fn default_value_becomes_an_authored_named_argument() {
    let mut editor = editor("form controls {\n    show: presentation/tick\n}\n");
    let before = editor.expand_form("controls").unwrap();
    editor
        .set_gear_configuration(
            0,
            &before.expanded_form_id,
            "show",
            "maximum-values",
            ConfigurationValue::U64(3),
        )
        .unwrap();
    assert!(editor
        .view()
        .source
        .contains("show: presentation/tick(maximum-values = 3)"));
}

#[test]
fn invalid_and_stale_edits_are_immediate_atomic_refusals() {
    let mut editor =
        editor("form controls {\n    show: presentation/tick(maximum-values = 3)\n}\n");
    let before = editor.expand_form("controls").unwrap();
    let source = editor.view().source;
    let error = editor
        .set_gear_configuration(
            0,
            &before.expanded_form_id,
            "show",
            "maximum-values",
            ConfigurationValue::U64(5),
        )
        .unwrap_err();
    assert!(
        matches!(error, FormEditorError::InvalidConfiguration(message) if message.contains("1 through 4"))
    );
    assert_eq!(editor.view().source, source);
    let error = editor
        .set_gear_configuration(
            1,
            &before.expanded_form_id,
            "show",
            "maximum-values",
            ConfigurationValue::U64(8),
        )
        .unwrap_err();
    assert!(matches!(error, FormEditorError::StaleRevision { .. }));
    assert_eq!(editor.view().source, source);
}

#[test]
fn bounded_text_control_escapes_canonical_source() {
    let mut editor = editor("form controls {\n    literal: text/literal(\"hello\")\n}\n");
    let before = editor.expand_form("controls").unwrap();
    editor
        .set_gear_configuration(
            0,
            &before.expanded_form_id,
            "literal",
            "value",
            ConfigurationValue::Text("say \\\"hi\\\"".into()),
        )
        .unwrap();
    assert!(editor.view().source.contains("\"say \\\\\\\"hi\\\\\\\"\""));
    assert_eq!(
        editor.expand_form("controls").unwrap().gears[0].configuration[0].value,
        ConfigurationValue::Text("say \\\"hi\\\"".into())
    );
}

#[test]
fn finite_text_contract_projects_an_exact_choice_and_rejects_invented_values() {
    let mut editor = editor("form controls {\n    compare: logic/compare(operator = \"eq\")\n}\n");
    let before = editor.expand_form("controls").unwrap();
    let control = &PatchbayGraph::from_expanded(&before).unwrap().gears[0].controls[0];
    assert!(matches!(
        &control.kind,
        FaceControlKind::TextChoice { choices }
            if choices == &["lt", "le", "eq", "ne", "ge", "gt"]
    ));
    let source = editor.view().source;
    assert!(matches!(
        editor.set_gear_configuration(
            0,
            &before.expanded_form_id,
            "compare",
            "operator",
            ConfigurationValue::Text("contains".into()),
        ),
        Err(FormEditorError::InvalidConfiguration(message)) if message.contains("choose one of")
    ));
    assert_eq!(editor.view().source, source);
}
