//! Actual canonical sources through Crèche BIRTH and ordinary Body workload changes.
//! These tests do not claim browser interaction or a Body-wide executed Play.
use super::{initial_forms, session};

const BUTTON: &str = include_str!("../../../../../forms/button-across-room/main.conduit");
const CLOCK: &str = include_str!("../../../../../forms/clock/main.conduit");
const OTHER: &str = include_str!("../../../../../forms/desk-telegraph/main.conduit");

fn bundle(clock_entry: &str) -> String {
    serde_json::json!({
        "schema": "conduit.creche/reviewed-form-bundle@1",
        "forms": [
            {"slug": "button-across-room", "entry": "button_across_room", "source": BUTTON},
            {"slug": "clock", "entry": clock_entry, "source": CLOCK},
            {"slug": "desk-telegraph", "entry": "desk_telegraph", "source": OTHER},
        ],
    })
    .to_string()
}

#[test]
fn canonical_button_review_honors_the_selected_one_item_indicator_bound() {
    let inventory = initial_forms::reviewed_inventory(BUTTON).unwrap();
    let form = &inventory.forms[0];
    let selection = serde_json::to_string(&[initial_forms::InitialFormSelection {
        name: form.name.clone(),
        source_document_id: form.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
    }])
    .unwrap();
    let host = crate::installed_browser::advertisement(
        "review/button-host".into(),
        "review/button-boot".into(),
    );
    let review = super::review::review(
        BUTTON,
        &selection,
        &[host],
        &crate::installed_browser::local_bases(),
    )
    .unwrap();
    assert_eq!(review.selected_form_count, 1);
    assert!(!review.body_plan_created);
    assert!(!review.resources_acquired);
}

#[test]
fn canonical_button_clock_and_unrelated_form_are_one_initial_body_workset() {
    session::clear_for_test();
    let source = bundle("clock-demo");
    let inventory = initial_forms::reviewed_inventory(&source).unwrap();
    assert_eq!(inventory.forms.len(), 3);
    for (entry, canonical) in inventory.forms.iter().zip([BUTTON, CLOCK, OTHER]) {
        assert_eq!(entry.source, canonical);
        let standalone = initial_forms::check_source(canonical).unwrap();
        assert_eq!(
            entry.source_document_id,
            standalone.source_document_id.as_str()
        );
        assert_eq!(
            entry.checked_form_id,
            standalone.forms[0].checked_form_id.as_str()
        );
    }
    let selection: Vec<_> = inventory
        .forms
        .iter()
        .map(|form| initial_forms::InitialFormSelection {
            name: form.name.clone(),
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
        })
        .collect();
    let admitted = crate::source_interaction::admit_source(source.as_bytes(), 2256).unwrap();
    let receipt = session::birth(
        "browser/creche",
        "browser-boot/button-workset",
        "button and clock",
        &serde_json::to_string(&selection).unwrap(),
        &source,
        2256,
        admitted,
    )
    .unwrap();
    assert_eq!(receipt.raw_body.workset.len(), 3);
    assert_eq!(receipt.raw_body.workload_revision, 0);
    let button = conduit_body::ResidentForm::new(
        selection[0].source_document_id.as_str().into(),
        selection[0].checked_form_id.as_str().into(),
    );
    let removed = receipt
        .raw_body
        .remove_form(&button, "sign/remove-button".into())
        .unwrap();
    assert_eq!(removed.body_id, receipt.raw_body.body_id);
    assert_eq!(removed.workset.len(), 2);
    let restored = removed
        .admit_form(button, "sign/restore-button".into())
        .unwrap();
    assert_eq!(restored.body_id, receipt.raw_body.body_id);
    assert_eq!(restored.workset, receipt.raw_body.workset);
    assert_eq!(restored.workload_revision, 2);
    assert_eq!(receipt.raw_body.workload_revision, 0);
    session::clear_for_test();
}

#[test]
fn bundling_does_not_rename_the_canonical_clock_or_accept_a_substituted_entry() {
    assert!(initial_forms::reviewed_inventory(&bundle("clock"))
        .unwrap_err()
        .contains("mismatched"));
    let mut value: serde_json::Value = serde_json::from_str(&bundle("clock-demo")).unwrap();
    value["forms"][1]["entry"] = serde_json::json!("");
    assert!(initial_forms::reviewed_inventory(&value.to_string()).is_err());
}
