#![cfg(feature = "authoring")]

use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_todo_plot::{
    install_todo_catalogs, TODO_CHECKPOINT_KIND, TODO_CHECKPOINT_READ_KIND, TODO_COMBINE_KIND,
};

#[test]
fn one_command_plot_routes_reducer_through_checkpoint_before_output() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_todo_catalogs(&mut startup, &mut profile, "Groceries").unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../checkpoint-once.conduit")),
        &startup,
    )
    .unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "todo/checkpoint-once", &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 2);
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == TODO_COMBINE_KIND));
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == TODO_CHECKPOINT_KIND));
    expanded.validate_expansion().unwrap();
}

#[test]
fn restore_plot_has_one_typed_read_source() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_todo_catalogs(&mut startup, &mut profile, "Groceries").unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../checkpoint-restore.conduit")),
        &startup,
    )
    .unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "todo/checkpoint-restore", &profile)
            .unwrap()
            .expanded;
    assert_eq!(expanded.gears.len(), 1);
    assert_eq!(
        expanded.gears[0].kind_id.as_str(),
        TODO_CHECKPOINT_READ_KIND
    );
    expanded.validate_expansion().unwrap();
}
