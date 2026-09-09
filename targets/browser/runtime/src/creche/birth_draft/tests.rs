use super::*;
use conduit_presentation::{ApplicationComponent, ApplicationView};
use serde_json::{json, Value};

fn open_request() -> OpenRequest {
    OpenRequest {
        persona_uuid: "11111111-2222-4333-8444-555555555555".into(),
        choices: vec![Choice {
            title: "Memory Lantern".into(),
            search_text: "state text".into(),
            form: ResidentForm {
                source_document_id: "source/lantern".into(),
                checked_form_id: "checked/lantern".into(),
            },
            selected: false,
        }],
    }
}
fn output() -> Vec<u8> {
    // The ABI owns these buffers on this test thread; copy before the next call.
    unsafe {
        std::slice::from_raw_parts(
            abi::conduit_creche_output_ptr() as *const u8,
            abi::conduit_creche_output_len(),
        )
        .to_vec()
    }
}
fn snapshot() -> Value {
    serde_json::from_slice(&output()).unwrap()
}
fn event(current: &CurrentDraft, action: &str, event: &str, value: &str) -> EventRequest {
    EventRequest {
        generation: current.generation,
        revision: current.draft.as_ref().unwrap().revision(),
        action: action.into(),
        event: event.into(),
        value: value.into(),
    }
}

#[test]
fn shared_draft_edits_and_birth_request_do_not_create_lifecycle_truth() {
    super::super::session::clear_for_test();
    let mut current = CurrentDraft::default();
    current.open(open_request()).unwrap();
    current
        .event(event(&current, "creche.naming", "change", "roman"))
        .unwrap();
    current
        .event(event(&current, "creche.name", "input", "Juniper"))
        .unwrap();
    current
        .event(event(&current, "creche.form.0", "change", "true"))
        .unwrap();
    current
        .event(event(&current, "creche.birth", "activate", ""))
        .unwrap();
    let result = snapshot();
    assert_eq!(result["friendly_name"], "Juniper");
    assert_eq!(result["naming_system"], "roman");
    assert_eq!(
        result["selected"],
        json!([{
            "source_document_id": "source/lantern", "checked_form_id": "checked/lantern"
        }])
    );
    assert_eq!(result["birth_requested"], true);
    assert!(super::super::session::current().is_none());
}

#[test]
fn replaced_generation_and_stale_or_malformed_events_leave_draft_unchanged() {
    let mut current = CurrentDraft::default();
    current.open(open_request()).unwrap();
    let old = event(&current, "creche.name", "input", "old");
    current.open(open_request()).unwrap();
    assert!(current.event(old).unwrap_err().contains("generation"));
    let old_revision = event(&current, "creche.name", "input", "old");
    current
        .event(event(&current, "creche.name", "input", "Juniper"))
        .unwrap();
    assert!(current
        .event(old_revision)
        .unwrap_err()
        .contains("StalePresentation"));
    for (action, kind, value) in [
        ("creche.form.0", "change", "yes"),
        ("creche.form.01", "change", "true"),
        ("creche.naming", "change", "Roman tria nomina"),
        ("creche.name", "activate", "wrong kind"),
        ("creche.birth", "activate", "invented payload"),
    ] {
        let revision = current.draft.as_ref().unwrap().revision();
        assert!(current.event(event(&current, action, kind, value)).is_err());
        assert_eq!(current.draft.as_ref().unwrap().revision(), revision);
        assert_eq!(current.draft.as_ref().unwrap().friendly_name(), "Juniper");
    }
}

#[test]
fn oversized_inventory_is_refused_before_replacing_the_current_draft() {
    let mut current = CurrentDraft::default();
    current.open(open_request()).unwrap();
    let generation = current.generation;
    let name = current.draft.as_ref().unwrap().friendly_name().to_string();
    let mut oversized = open_request();
    oversized.choices = (0..16)
        .map(|index| Choice {
            title: format!("Form {index}"),
            search_text: String::new(),
            selected: false,
            form: ResidentForm {
                source_document_id: format!("{index:02}{}", "s".repeat(2046)).into(),
                checked_form_id: format!("{index:02}{}", "c".repeat(2046)).into(),
            },
        })
        .collect();
    assert!(current
        .open(oversized)
        .unwrap_err()
        .contains("InvalidInventory"));
    assert_eq!(current.generation, generation);
    assert_eq!(current.draft.as_ref().unwrap().friendly_name(), name);
}

#[test]
fn abi_view_is_the_encoded_shared_presentation_and_rejects_stale_mounts() {
    CURRENT
        .with(|slot| slot.borrow_mut().open(open_request()))
        .unwrap();
    let generation = snapshot()["generation"].as_u64().unwrap() as u32;
    assert_eq!(conduit_creche_birth_draft_view(generation), 0);
    let view = ApplicationView::decode(&output()).unwrap();
    assert!(view
        .nodes
        .iter()
        .any(|node| node.component == ApplicationComponent::Heading
            && node.text == "A Body of your own"));
    assert!(view
        .nodes
        .iter()
        .any(|node| node.component == ApplicationComponent::Option
            && node.value == "roman"
            && node.text == "Roman tria nomina"));
    CURRENT
        .with(|slot| slot.borrow_mut().open(open_request()))
        .unwrap();
    assert_eq!(conduit_creche_birth_draft_view(generation), ERROR_DRAFT);
    assert_eq!(snapshot()["disposition"], "refused-before-lifecycle-change");
}
