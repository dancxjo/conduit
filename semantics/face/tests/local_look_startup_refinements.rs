//! Minimal owning-checker regression discovered while authoring a personal Look.
//! The same retained sequence/variant law must govern defaults and explicit calls.
use conduit_plot::{
    check_syntax_document, parse_syntax_document, KindSignature, StartupCatalog,
    StartupParameterSignature,
};
#[test]
fn default_and_explicit_startup_retain_variant_sequence_owner_laws() {
    let declarations="type PixelOverride =\n body U16 in 8..=64\n\ntype Personal = {\n values: sequence PixelOverride <= 1\n}\n";
    let owner = check_syntax_document(&parse_syntax_document(declarations), &StartupCatalog::new())
        .unwrap();
    let ty = owner
        .native_types
        .iter()
        .find(|ty| ty.name == "Personal")
        .unwrap();
    println!("retained contracts: {:?}", ty.value_contracts);
    let mut catalog = StartupCatalog::new();
    catalog.insert_checked_native_type("Personal", ty).unwrap();
    catalog
        .insert(KindSignature {
            kind: "fixture/personal".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "personal".into(),
                value_type: "Personal".into(),
                default: None,
            }],
        })
        .unwrap();
    let mut failures = vec![];
    for (form, template) in [
        (
            "default",
            "plot proposal (\n personal: Personal = {values: [body(VALUE)]}\n) {\n}\n",
        ),
        (
            "explicit",
            "plot proposal {\n use: fixture/personal(personal = {values: [body(VALUE)]})\n}\n",
        ),
    ] {
        for value in [18, 7] {
            let source = template.replace("VALUE", &value.to_string());
            let result = check_syntax_document(&parse_syntax_document(&source), &catalog);
            println!(
                "{form}/{value}: {:?}",
                result
                    .as_ref()
                    .map(|_| "accepted")
                    .map_err(|e| (e.code, e.message.as_str()))
            );
            if result.is_ok() != (value == 18) {
                failures.push((form, value));
            }
        }
    }
    assert!(failures.is_empty(), "owner startup laws lost: {failures:?}");
}
