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
    assert_eq!(checks.len(), 43);
    assert!(checks.iter().all(|result| result.status == "passed"));
    let measurement_window = checks
        .iter()
        .find(|result| result.slug == "measurement-window")
        .expect("measurement-window is explicitly reviewed");
    assert_eq!(measurement_window.title, "Measurement Window");
    assert_eq!(measurement_window.form_entry, "measurement-window");
    assert_eq!(
        measurement_window.source_path,
        "forms/measurement-window/main.conduit"
    );
    assert!(measurement_window.source_document_id.is_some());
    assert!(measurement_window.checked_form_id.is_some());
    let button = checks
        .iter()
        .find(|result| result.slug == "button-across-room")
        .expect("reviewed Button Across the Room result");
    assert_eq!(button.title, "Button Across the Room");
    assert_eq!(button.source_path, "forms/button-across-room/main.conduit");
    assert_eq!(button.form_entry, "button_across_room");
    assert!(button.source_document_id.is_some());
    assert!(button.checked_form_id.is_some());
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
    let bundle: serde_json::Value = serde_json::from_str(&source).unwrap();
    assert_eq!(bundle["schema"], "conduit.creche/reviewed-form-bundle@1");
    assert_eq!(bundle["forms"].as_array().unwrap().len(), 3);
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

#[test]
fn one_source_failure_does_not_prevent_later_inventory_results() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "conduit-form-failure-isolation-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("forms/bad")).unwrap();
    fs::create_dir_all(root.join("forms/good")).unwrap();
    fs::write(
        root.join("forms/inventory.toml"),
        r#"schema = "conduit.reviewed-form-inventory/v1"
maximum_forms = 2
maximum_combined_workloads = 1

[[forms]]
slug = "bad"
title = "Bad"
entry = "bad"

[[forms]]
slug = "good"
title = "Good"
entry = "good"
deterministic_not_applicable = "checking-only fixture"
browser_safe_not_applicable = "checking-only fixture"

[[combined_workloads]]
slug = "fixture-workload"
title = "Fixture Workload"
workload_revision = 0
entries = [
  { slug = "bad", entry = "bad" },
  { slug = "good", entry = "good" },
]
deterministic = { package = "fixture", test = "fixture", case = "fixture", plan_play_evidence = true, workload_revision_evidence = true }
"#,
    )
    .unwrap();
    fs::write(root.join("forms/bad/main.conduit"), "form bad {\n").unwrap();
    fs::write(root.join("forms/good/main.conduit"), "form good {\n}\n").unwrap();

    let report = build_report(&root, false, &GlobalOpts::default()).unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert!(report.results.iter().any(|result| {
        result.slug == "bad" && result.proof_mode == "check" && result.status == "failed"
    }));
    assert!(report.results.iter().any(|result| {
        result.slug == "good" && result.proof_mode == "check" && result.status == "passed"
    }));
    assert!(report.results.iter().any(|result| {
        result.slug == "good"
            && result.proof_mode == "deterministic"
            && result.status == "not_applicable"
    }));
}

#[test]
fn combined_workload_is_projected_without_static_execution_claims() {
    let root = crate::workspace::workspace_root().unwrap();
    let report = build_report(&root, false, &GlobalOpts::default()).unwrap();
    let combined = report
        .results
        .iter()
        .filter(|result| result.proof_mode == "combined-deterministic")
        .collect::<Vec<_>>();
    assert_eq!(combined.len(), 3);
    assert_eq!(
        combined
            .iter()
            .map(|result| result.slug.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["desk-telegraph", "memory-lantern", "morse-network"])
    );
    assert!(combined.iter().all(|result| {
        result.status == "unavailable"
            && result.workload_slug.as_deref() == Some("initial-three-form-body")
            && result.workload_title.as_deref() == Some("Initial Three-Form Body")
            && result.duration_millis == 0
            && result.workload_revision.is_none()
            && result.plan_id.is_none()
            && result.play_id.is_none()
            && result.source_document_id.is_some()
            && result.checked_form_id.is_some()
    }));
}
