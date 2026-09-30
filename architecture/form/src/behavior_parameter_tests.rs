use crate::{check_syntax_document, parse_syntax_document, CheckedSyntaxDocument, StartupCatalog};

fn check(source: &str) -> CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    check_syntax_document(&parsed, &StartupCatalog::new()).expect("behavior source checks")
}

#[test]
fn type_and_behavior_parameters_specialize_one_exact_generic_fore() {
    let checked = check(
        "form count/identity (\n value: Count >> result: Count\n) {\n value >> result\n}\n\nform apply (\n item: type\n transform: kind (\n  value: item >> result: item\n )\n value: item >> result: item\n) {\n value >> transform() >> result\n}\n\nform main {\n applied: apply(item = Count, transform = count/identity)\n}\n",
    );
    let specialized = checked
        .forms
        .iter()
        .find(|form| form.name == "apply[item=value/count,transform=count/identity]")
        .unwrap();
    assert_eq!(specialized.gears[0].kind, "count/identity");
    assert_eq!(
        specialized.runtime_front.inputs()[0].value_kind,
        conduit_core::kind_id("value/count")
    );
}

#[test]
fn local_form_captures_outer_behavior_only_as_compile_time_identity() {
    let checked = check(
        "form text/upper (\n value: Text >> mapped: Text\n) {\n value >> mapped\n}\n\nform outer (\n transform: kind (\n  value: Text >> mapped: Text\n )\n value: Text >> mapped: Text\n) {\n form helper (\n  value: Text >> mapped: Text\n ) {\n  value >> transform() >> mapped\n }\n helper: helper\n value >> helper >> mapped\n}\n\nform main {\n applied: outer(transform = text/upper)\n}\n",
    );
    let outer = checked
        .forms
        .iter()
        .find(|form| form.name == "outer[transform=text/upper]")
        .unwrap();
    let helper = checked
        .forms
        .iter()
        .find(|form| form.name == "$local/outer/helper[transform=text/upper]")
        .unwrap();
    assert_eq!(outer.gears[0].kind, helper.name);
    assert_eq!(helper.gears[0].kind, "text/upper");
}

#[test]
fn exact_behavior_identity_changes_checked_specialization_identity() {
    let checked = check(
        "form text/first (\n value: Text >> mapped: Text\n) {\n value >> mapped\n}\n\nform text/second (\n value: Text >> mapped: Text\n) {\n value >> mapped\n}\n\nform apply (\n transform: kind (\n  value: Text >> mapped: Text\n )\n value: Text >> mapped: Text\n) {\n value >> transform() >> mapped\n}\n\nform main {\n first: apply(transform = text/first)\n second: apply(transform = text/second)\n}\n",
    );
    let first = checked
        .forms
        .iter()
        .find(|form| form.name == "apply[transform=text/first]")
        .unwrap();
    let second = checked
        .forms
        .iter()
        .find(|form| form.name == "apply[transform=text/second]")
        .unwrap();
    assert_ne!(first.checked_form_id, second.checked_form_id);
    assert_eq!(first.gears[0].kind, "text/first");
    assert_eq!(second.gears[0].kind, "text/second");
}
