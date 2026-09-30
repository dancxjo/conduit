use crate::{
    ProfileCatalog, StartupCatalog, check_syntax_document, expand_canonical_form_for_authoring,
    parse_syntax_document,
};

const SOURCE: &str = "
form text/normalize (
 >> value: Text
 mapped: Text >>
) {
 value >> mapped
}

form flow/each (
 item: type
 result: type
 transform: kind (
  >> value: item
  mapped: result >>
 )
 >> values: item...|
 mapped: result...| >>
) {
 each: activate(maximum-items = 4) transform()
 values >> each.value
 each.mapped >> mapped
}

form main {
 mapped: flow/each(item = Text, result = Text, transform = text/normalize)
}
";

const SELECT_SOURCE: &str = "
form text/is-useful (
 >> value: Text
 accepted: Boolean >>
) {
}

form flow/select (
 item: type
 predicate: kind (
  >> value: item
  accepted: Boolean >>
 )
 >> values: item...|
 selected: item...| >>
) {
 selection: select(maximum-items = 4) predicate()
 values >> selection.value
 selection.selected >> selected
}

form main {
 selected: flow/select(item = Text, predicate = text/is-useful)
}
";

const FOLD_SOURCE: &str = "
form integer/add (
 >> accumulator: U64
 >> item: U64
 combined: U64 >>
) {
}

form flow/fold-integers (
 initial: U64
 >> items: U64...|
 result: U64 >>
) {
 folder: fold(initial, maximum-items = 4) integer/add()
 items >> folder.item
 folder.combined >> result
}

form main {
 folded: flow/fold-integers(initial = 7)
}
";

fn checked() -> crate::CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap()
}

#[test]
fn specialization_retains_one_exact_checked_activation() {
    let checked = checked();
    let each = checked
        .forms
        .iter()
        .find(|form| form.name.starts_with("flow/each["))
        .unwrap();
    let activation = each.gears[0].activation.as_ref().unwrap();
    assert_eq!(activation.selected_form, "text/normalize");
    assert_eq!(activation.input.temporal, conduit_core::PortTemporal::Value);
    assert_eq!(
        activation.output.temporal,
        conduit_core::PortTemporal::Value
    );
}

#[test]
fn activation_law_participates_in_checked_identity() {
    let activated = checked();
    let ordinary_source = SOURCE.replace("activate(maximum-items = 4) transform()", "transform()");
    let ordinary = check_syntax_document(
        &parse_syntax_document(&ordinary_source),
        &StartupCatalog::new(),
    )
    .unwrap();
    let identity = |document: &crate::CheckedSyntaxDocument| {
        document
            .forms
            .iter()
            .find(|form| form.name.starts_with("flow/each["))
            .unwrap()
            .checked_form_id
            .clone()
    };
    assert_ne!(identity(&activated), identity(&ordinary));
}

#[test]
fn expansion_keeps_selected_form_opaque_and_emits_flow_coordinator() {
    let checked = checked();
    let name = checked
        .forms
        .iter()
        .find(|form| form.name.starts_with("flow/each["))
        .unwrap()
        .name
        .clone();
    let expanded = expand_canonical_form_for_authoring(&checked, &name, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    assert_eq!(expanded.gears[0].kind_id.as_str(), "flow/each");
    assert_eq!(expanded.activations.len(), 1);
    assert_eq!(expanded.activations[0].selected_form, "text/normalize");
    assert_eq!(
        expanded.gears[0].inputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        expanded.gears[0].outputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert!(expanded.validate_expansion().is_ok());

    let mut stale = expanded.clone();
    stale.activations[0].selected_checked_form_id = "other-checked-form".into();
    assert!(stale.validate_expansion().is_err());
}

#[test]
fn activation_refuses_a_flow_behavior() {
    let source = SOURCE.replace(
        ">> value: Text\n mapped: Text >>",
        ">> value: Text...|\n mapped: Text...| >>",
    );
    let error =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-057");
}

#[test]
fn select_specialization_keeps_the_exact_predicate_but_outputs_the_retained_item() {
    let checked = check_syntax_document(
        &parse_syntax_document(SELECT_SOURCE),
        &StartupCatalog::new(),
    )
    .unwrap();
    let select = checked
        .forms
        .iter()
        .find(|form| form.name.starts_with("flow/select["))
        .unwrap();
    let activation = select.gears[0].activation.as_ref().unwrap();
    assert_eq!(
        activation.mode,
        crate::ActivationSyntax::Select { maximum_items: 4 }
    );
    assert_eq!(
        activation.input.value_kind.as_str(),
        conduit_core::TEXT_INFO_ID
    );
    assert_eq!(
        activation.output.value_kind.as_str(),
        conduit_core::BOOL_INFO_ID
    );

    let expanded =
        expand_canonical_form_for_authoring(&checked, &select.name, &ProfileCatalog::new())
            .unwrap()
            .expanded;
    assert_eq!(expanded.gears[0].kind_id.as_str(), "flow/select");
    assert_eq!(
        expanded.gears[0].inputs[0].value_kind.as_str(),
        conduit_core::TEXT_INFO_ID
    );
    assert_eq!(expanded.gears[0].outputs[0].port_id.as_str(), "selected");
    assert_eq!(
        expanded.gears[0].outputs[0].value_kind.as_str(),
        conduit_core::TEXT_INFO_ID
    );
    assert_eq!(
        expanded.activations[0].output.value_kind.as_str(),
        conduit_core::BOOL_INFO_ID
    );
    expanded.validate_expansion().unwrap();
}

#[test]
fn select_refuses_non_boolean_predicates_without_truthiness_coercion() {
    let source = SELECT_SOURCE.replace("accepted: Boolean", "accepted: Text");
    let error =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-063");
}

#[test]
fn fold_retains_exact_initial_and_two_input_combine_truth() {
    let checked =
        check_syntax_document(&parse_syntax_document(FOLD_SOURCE), &StartupCatalog::new()).unwrap();
    let fold = checked
        .forms
        .iter()
        .find(|form| form.name == "flow/fold-integers")
        .unwrap();
    let activation = fold.gears[0].activation.as_ref().unwrap();
    assert!(matches!(
        activation.mode,
        crate::ActivationSyntax::Fold { .. }
    ));
    assert_eq!(activation.input.port_id.as_str(), "item");
    assert_eq!(
        activation
            .accumulator_input
            .as_ref()
            .unwrap()
            .port_id
            .as_str(),
        "accumulator"
    );
    assert_eq!(activation.output.port_id.as_str(), "combined");
    assert_eq!(
        activation.initial_accumulator,
        Some(crate::CanonicalStartupValue::FormParameter(
            "initial".into()
        ))
    );

    let expanded = crate::expand_canonical_form(&checked, "main", &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.gears[0].kind_id.as_str(), "flow/fold");
    assert_eq!(
        expanded.gears[0].inputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        expanded.gears[0].outputs[0].temporal,
        conduit_core::PortTemporal::Value
    );
    assert!(expanded.activations[0].accumulator_input.is_some());
    assert_eq!(
        expanded.activations[0].initial_accumulator,
        Some(crate::CanonicalStartupValue::Literal("7".into()))
    );
    assert_eq!(
        expanded.activations[0].initial_accumulator_bytes,
        Some(7_u64.to_le_bytes().to_vec())
    );
    expanded.validate_expansion().unwrap();

    let mut unresolved = expanded.clone();
    unresolved.activations[0].initial_accumulator = Some(
        crate::CanonicalStartupValue::FormParameter("initial".into()),
    );
    assert!(unresolved.validate_expansion().is_err());
}

#[test]
fn fold_refuses_a_unary_combine_form() {
    let source = FOLD_SOURCE.replace(
        ">> accumulator: U64\n >> item: U64\n combined: U64 >>",
        ">> item: U64\n combined: U64 >>",
    );
    let error =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-064");
}
