//! Exact atomic-argument signatures in required component slots.
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use sha2::{Digest, Sha256};

fn face(minimum: u32) -> Face {
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "item".into(),
            role: FaceRole::Item,
            name: "Item".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![FaceAction {
            identity: "edit".into(),
            intent: "example/edit@1".into(),
            target: "item".into(),
            name: "Edit".into(),
            arguments: vec![
                FaceActionArgument::text("value".into(), "Value".into(), minimum, 32).unwrap(),
            ],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        vec![FaceDisclosure {
            subject: "item".into(),
            level: FaceDisclosureLevel::Primary,
        }],
    )
    .unwrap()
}
fn action(source: &Face, argument_name: &str, value_kind: &str) -> ComponentActionBinding {
    let digest = Sha256::digest(source.actions[0].arguments[0].contract.identity_bytes());
    ComponentActionBinding::new(
        BoundedSequence::try_from_iter([ComponentArgumentBinding::new(
            format!("{digest:x}"),
            argument_name.into(),
            value_kind.into(),
        )
        .unwrap()])
        .unwrap(),
        "edit".into(),
        "example/edit@1".into(),
    )
    .unwrap()
}
fn recipe(actions: Vec<ComponentActionBinding>) -> conduit_core::StructuredInfoValue {
    let selector = ComponentSubjectSelector::bound(
        BoundedSequence::try_from_iter(actions).unwrap(),
        BoundedSequence::new(),
        ComponentBindingRole::role("Item".into()).unwrap(),
        "item".into(),
    )
    .unwrap();
    ComponentRecipeInfo::new(
        12,
        "test/atomic@1".into(),
        1,
        720,
        BoundedSequence::try_from_iter([
            ComponentRegionInfo::new(0, "item".into(), 1, selector).unwrap()
        ])
        .unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap()
}
#[test]
fn exact_checked_argument_contract_is_required_not_just_kind_or_capacity() {
    let original = face(0);
    let bound = action(&original, "value", UTF8_TEXT_VALUE_KIND);
    prepare_component_recipe(recipe(vec![bound.clone()]), &original).unwrap();
    // Same Kind and maximum extent, but a different accepted source domain.
    let narrowed = face(1);
    assert_eq!(
        prepare_component_recipe(recipe(vec![bound]), &narrowed).err(),
        Some(ComponentRecipeRefusal::SlotAction)
    );
    for (name, kind) in [("other", UTF8_TEXT_VALUE_KIND), ("value", "value/bool")] {
        assert_eq!(
            prepare_component_recipe(recipe(vec![action(&original, name, kind)]), &original).err(),
            Some(ComponentRecipeRefusal::SlotAction)
        );
    }
    let missing = ComponentActionBinding::new(
        BoundedSequence::new(),
        "edit".into(),
        "example/edit@1".into(),
    )
    .unwrap();
    assert_eq!(
        prepare_component_recipe(recipe(vec![missing]), &original).err(),
        Some(ComponentRecipeRefusal::SlotAction)
    );
}
#[test]
fn duplicate_required_actions_and_over_capacity_contracts_refuse() {
    let source = face(0);
    let binding = action(&source, "value", UTF8_TEXT_VALUE_KIND);
    assert_eq!(
        prepare_component_recipe(recipe(vec![binding.clone(), binding.clone()]), &source).err(),
        Some(ComponentRecipeRefusal::DuplicateBinding)
    );
    assert!(
        BoundedSequence::<ComponentActionBinding, 2>::try_from_iter([
            binding.clone(),
            binding.clone(),
            binding
        ])
        .is_err()
    );
}
