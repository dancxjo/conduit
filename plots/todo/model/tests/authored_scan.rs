#![cfg(feature = "authoring")]

use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_todo_plot::{install_todo_catalogs, TodoState, TODO_COMBINE_KIND, TODO_STATE_INFO_ID};

const SOURCE: &str = include_str!("../../live.conduit");

#[test]
fn authored_todo_scan_carries_owner_validated_exact_initial_form() {
    let mut catalog = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_todo_catalogs(&mut catalog, &mut profiles, "Groceries").unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &catalog).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "todo/main", &profiles)
        .unwrap()
        .expanded;
    let activation = &expanded.activations[0];
    assert_eq!(activation.mode.maximum_items(), 64);
    assert_eq!(
        activation
            .accumulator_input
            .as_ref()
            .unwrap()
            .value_kind
            .as_str(),
        TODO_STATE_INFO_ID
    );
    let initial = activation.initial_accumulator_bytes.as_ref().unwrap();
    assert_eq!(
        TodoState::decode_info(initial).unwrap(),
        TodoState::new("Groceries".into()).unwrap()
    );
    expanded.validate_expansion().unwrap();
}

#[test]
fn missing_or_wrong_kind_initial_form_is_refused() {
    let missing = kind_only_catalog();
    let refusal = check_syntax_document(&parse_syntax_document(SOURCE), &missing).unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-064");

    let mut invalid = kind_only_catalog();
    assert!(invalid
        .insert_exact_initial_info(
            conduit_core::kind_id(TODO_STATE_INFO_ID),
            "\"Groceries\"",
            vec![0xff],
            |bytes| TodoState::decode_info(bytes).is_ok(),
        )
        .is_err());

    let mut wrong = kind_only_catalog();
    wrong
        .insert_exact_initial_info(
            conduit_core::kind_id("conduit.todo/other@1"),
            "\"Groceries\"",
            TodoState::new("Groceries".into())
                .unwrap()
                .encode_info()
                .unwrap(),
            |bytes| TodoState::decode_info(bytes).is_ok(),
        )
        .unwrap();
    let refusal = check_syntax_document(&parse_syntax_document(SOURCE), &wrong).unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-064");
}

fn kind_only_catalog() -> StartupCatalog {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: TODO_COMBINE_KIND.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
}
