use crate::{check_syntax_document, parse_syntax_document, CheckedSyntaxDocument, StartupCatalog};

fn check(source: &str) -> CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    check_syntax_document(&parsed, &StartupCatalog::new()).expect("behavior source checks")
}

#[test]
fn type_and_behavior_parameters_specialize_one_exact_generic_fore() {
    let checked = check(
        "plot count/identity (\n value: Count >> result: Count\n) {\n value >> result\n}\n\nplot apply (\n item: type\n transform: kind (\n  value: item >> result: item\n )\n value: item >> result: item\n) {\n value >> transform() >> result\n}\n\nplot main {\n applied: apply(item = Count, transform = count/identity)\n}\n",
    );
    let specialized = checked
        .plots
        .iter()
        .find(|plot| plot.name == "apply[item=value/count,transform=count/identity]")
        .unwrap();
    assert_eq!(specialized.gears[0].kind, "count/identity");
    assert_eq!(
        specialized.runtime_front.inputs()[0].value_kind,
        conduit_core::kind_id("value/count")
    );
}

#[test]
fn local_plot_captures_outer_behavior_only_as_compile_time_identity() {
    let checked = check(
        "plot text/upper (\n value: Text >> mapped: Text\n) {\n value >> mapped\n}\n\nplot outer (\n transform: kind (\n  value: Text >> mapped: Text\n )\n value: Text >> mapped: Text\n) {\n plot helper (\n  value: Text >> mapped: Text\n ) {\n  value >> transform() >> mapped\n }\n helper: helper\n value >> helper >> mapped\n}\n\nplot main {\n applied: outer(transform = text/upper)\n}\n",
    );
    let outer = checked
        .plots
        .iter()
        .find(|plot| plot.name == "outer[transform=text/upper]")
        .unwrap();
    let helper = checked
        .plots
        .iter()
        .find(|plot| plot.name == "$local/outer/helper[transform=text/upper]")
        .unwrap();
    assert_eq!(outer.gears[0].kind, helper.name);
    assert_eq!(helper.gears[0].kind, "text/upper");
}

#[test]
fn exact_behavior_identity_changes_checked_specialization_identity() {
    let checked = check(
        "plot text/first (\n value: Text >> mapped: Text\n) {\n value >> mapped\n}\n\nplot text/second (\n value: Text >> mapped: Text\n) {\n value >> mapped\n}\n\nplot apply (\n transform: kind (\n  value: Text >> mapped: Text\n )\n value: Text >> mapped: Text\n) {\n value >> transform() >> mapped\n}\n\nplot main {\n first: apply(transform = text/first)\n second: apply(transform = text/second)\n}\n",
    );
    let first = checked
        .plots
        .iter()
        .find(|plot| plot.name == "apply[transform=text/first]")
        .unwrap();
    let second = checked
        .plots
        .iter()
        .find(|plot| plot.name == "apply[transform=text/second]")
        .unwrap();
    assert_ne!(first.checked_plot_id, second.checked_plot_id);
    assert_eq!(first.gears[0].kind, "text/first");
    assert_eq!(second.gears[0].kind, "text/second");
}
