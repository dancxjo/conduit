use super::*;
use std::collections::BTreeSet;

fn inventory_form() -> InventoryForm {
    InventoryForm {
        slug: "fixture".into(),
        title: "Fixture".into(),
        entry: "composition".into(),
        reusable_entries: Vec::new(),
        initial_body_order: None,
        deterministic: None,
        deterministic_not_applicable: None,
        browser_safe: None,
        browser_safe_not_applicable: None,
    }
}

#[test]
fn explicit_inventory_covers_canonical_sources_and_checks_every_entry() {
    let root = crate::workspace::workspace_root().unwrap();
    let report = build_report(&root, false, &GlobalOpts::default()).unwrap();
    let checks: Vec<_> = report
        .results
        .iter()
        .filter(|result| result.proof_mode == "check")
        .collect();
    assert_eq!(checks.len(), 35);
    assert!(checks.iter().all(|result| result.status == "passed"));
    assert!(report
        .results
        .iter()
        .all(|result| result.status != "skipped"));
    let reusable: Vec<_> = report
        .results
        .iter()
        .filter(|result| result.proof_mode == "reusable-check")
        .collect();
    assert_eq!(reusable.len(), 10);
    assert!(reusable.iter().all(|result| {
        result.status == "passed"
            && result.source_document_id.is_some()
            && result.checked_form_id.is_some()
    }));
    assert_eq!(
        reusable
            .iter()
            .map(|result| (&result.source_path, &result.form_entry))
            .collect::<BTreeSet<_>>()
            .len(),
        reusable.len()
    );
    let composition: Vec<_> = report
        .results
        .iter()
        .filter(|result| result.proof_mode == "composition-check")
        .collect();
    assert_eq!(composition.len(), 10);
    assert_eq!(
        composition
            .iter()
            .filter(|result| result.status == "passed")
            .count(),
        9
    );
    assert_eq!(
        composition
            .iter()
            .filter(|result| result.status == "unavailable")
            .count(),
        1
    );
    assert!(composition
        .iter()
        .all(|result| { result.source_document_id.is_some() && result.checked_form_id.is_some() }));
    assert!(composition
        .iter()
        .filter(|result| result.status == "passed")
        .all(|result| result.composition_root_checked_form_id.is_some()
            && result.composition_root_entry.is_some()
            && !result.gear_occurrences.is_empty()));
    let reusable_deterministic: Vec<_> = report
        .results
        .iter()
        .filter(|result| result.proof_mode == "reusable-deterministic")
        .collect();
    assert_eq!(reusable_deterministic.len(), 10);
    assert!(reusable_deterministic
        .iter()
        .all(|result| result.status == "unavailable"));
}

#[test]
fn composition_check_refuses_an_inexact_occurrence_declaration() {
    let root = crate::workspace::workspace_root().unwrap();
    let mut inventory = load_inventory(&root).unwrap();
    let form = inventory
        .forms
        .iter_mut()
        .find(|form| form.slug == "count")
        .unwrap();
    form.reusable_entries[0]
        .composition
        .as_mut()
        .unwrap()
        .occurrences = vec!["invented".into()];
    let results = composition::check_all(
        &root,
        form,
        "forms/count/main.conduit",
        &catalogs().unwrap(),
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, "failed");
    assert!(results[0].reason.contains("expected {\"invented\"}"));
}

#[test]
fn initial_body_bundle_is_selected_by_the_shared_inventory() {
    let root = crate::workspace::workspace_root().unwrap();
    let output = std::env::temp_dir().join(format!(
        "conduit-initial-forms-{}.conduit",
        std::process::id()
    ));
    bundle_initial_body(&root, &output).unwrap();
    let source = fs::read_to_string(&output).unwrap();
    let _ = fs::remove_file(output);
    assert!(source.contains("form morse_network"));
    assert!(source.contains("form memory_lantern"));
    assert!(source.contains("form desk_telegraph"));
    assert_eq!(
        source.matches("\nform ").count() + usize::from(source.starts_with("form ")),
        3
    );
}

#[test]
fn reusable_entries_refuse_empty_duplicate_and_root_identities() {
    let mut form = inventory_form();
    form.reusable_entries.push(ReusableForm {
        entry: "helper".into(),
        title: "Helper".into(),
        composition: Some(CompositionOracle {
            parent: "composition".into(),
            occurrences: vec!["use".into()],
        }),
        deterministic: None,
    });
    assert!(inventory::reusable_entries_are_valid(&form));

    form.reusable_entries.push(ReusableForm {
        entry: "helper".into(),
        title: "Duplicate".into(),
        composition: None,
        deterministic: None,
    });
    assert!(!inventory::reusable_entries_are_valid(&form));
    form.reusable_entries[1].entry = "composition".into();
    assert!(!inventory::reusable_entries_are_valid(&form));
    form.reusable_entries[1].entry = "another".into();
    form.reusable_entries[1].title.clear();
    assert!(!inventory::reusable_entries_are_valid(&form));
    form.reusable_entries.truncate(1);
    form.reusable_entries[0]
        .composition
        .as_mut()
        .unwrap()
        .occurrences = vec!["use".into(), "use".into()];
    assert!(!inventory::reusable_entries_are_valid(&form));
}
