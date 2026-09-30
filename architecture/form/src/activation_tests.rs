use crate::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
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
 each: activate transform()
 values >> each.value
 each.mapped >> mapped
}

form main {
 mapped: flow/each(item = Text, result = Text, transform = text/normalize)
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
    let ordinary_source = SOURCE.replace("activate transform()", "transform()");
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
