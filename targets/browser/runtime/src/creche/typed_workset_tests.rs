//! Reviewed presentation profiles select existing exact typed realizations in one Body.
use super::{initial_forms, session};

fn bundle(profile: u8) -> String {
    serde_json::json!({
        "schema": "conduit.creche/reviewed-form-bundle@1",
        "forms": [
            {"slug": "memory-lantern", "entry": "memory_lantern", "source": include_str!("../../../../../forms/memory-lantern/main.conduit")},
            {"slug": "pocket-theremin", "entry": "pocket-theremin", "presentation_profile": profile, "source": include_str!("../../../../../forms/pocket-theremin/main.conduit")},
            {"slug": "secret-knock", "entry": "secret-knock-demo", "presentation_profile": 3, "source": include_str!("../../../../../forms/secret-knock/main.conduit")}
        ]
    }).to_string()
}

#[test]
fn typed_inventory_refuses_unknown_profiles_and_incompatible_default_types() {
    assert!(initial_forms::reviewed_inventory(&bundle(255))
        .unwrap_err()
        .contains("unsupported presentation profile"));
    let incompatible = bundle(0);
    let entries = initial_forms::reviewed_inventory(&incompatible).unwrap();
    let selection: Vec<_> = entries
        .forms
        .iter()
        .map(|entry| initial_forms::InitialFormSelection {
            name: entry.name.clone(),
            source_document_id: entry.source_document_id.clone(),
            checked_form_id: entry.checked_form_id.clone(),
        })
        .collect();
    let host = initial_forms::reviewed_browser_host(
        &incompatible,
        "host/typed".into(),
        "boot/typed".into(),
    )
    .unwrap();
    assert!(super::review::review(
        &incompatible,
        &serde_json::to_string(&selection).unwrap(),
        &[host],
        &crate::installed_browser::local_bases()
    )
    .is_err());
    let inventory = initial_forms::reviewed_inventory(&bundle(1)).unwrap();
    assert_eq!(inventory.forms.len(), 3);
    assert!(inventory
        .forms
        .iter()
        .all(|entry| !entry.required_kinds.is_empty()));
}

#[test]
fn text_quantity_and_pattern_presentations_plan_together_under_one_body() {
    session::clear_for_test();
    let source = bundle(1);
    let inventory = initial_forms::reviewed_inventory(&source).unwrap();
    let selected: Vec<_> = inventory
        .forms
        .iter()
        .map(|form| initial_forms::InitialFormSelection {
            name: form.name.clone(),
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
        })
        .collect();
    let interaction = crate::source_interaction::admit_source(source.as_bytes(), 2260).unwrap();
    let receipt = session::birth(
        "browser/typed-workset",
        "boot/typed-workset",
        "Many voices",
        &serde_json::to_string(&selected).unwrap(),
        &source,
        2260,
        interaction,
    )
    .unwrap();
    assert_eq!(receipt.initial_forms.len(), 3);
    #[cfg(feature = "form-runner")]
    {
        let plans = super::workspace::plan_workspace_forms(
            &session::biography().unwrap(),
            &source,
            &"browser/typed-workset".into(),
            &"boot/typed-workset".into(),
        )
        .unwrap();
        assert_eq!(plans.len(), 3);
        let selector = plans
            .iter()
            .flat_map(|part| &part.plan.fragments)
            .flat_map(|fragment| &fragment.placements)
            .find(|gear| {
                gear.implementation_id.as_str()
                    == crate::installed_browser::structured_selector::IMPLEMENTATION
            })
            .unwrap();
        let installed =
            crate::installed_browser::structured_selector::offer_for_placement(selector)
                .unwrap()
                .unwrap();
        assert_eq!(installed.capability_id, selector.capability_id);
        let mut substituted = selector.clone();
        substituted.capability_id = "selector/substituted".into();
        assert!(
            crate::installed_browser::structured_selector::offer_for_placement(&substituted)
                .is_err()
        );
        substituted = selector.clone();
        substituted.outputs.clear();
        assert!(
            crate::installed_browser::structured_selector::offer_for_placement(&substituted)
                .is_err()
        );
        substituted = selector.clone();
        substituted.configuration.clear();
        assert!(
            crate::installed_browser::structured_selector::offer_for_placement(&substituted)
                .is_err()
        );

        for (plan, form) in plans.iter().zip(receipt.raw_body.workset.forms()) {
            assert_eq!(&plan.form, form);
            assert!(plan
                .plan
                .fragments
                .iter()
                .all(|fragment| fragment.host_id.as_str() == "browser/typed-workset"));
        }
    }
    session::clear_for_test();
}
