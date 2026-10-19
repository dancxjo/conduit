//! Real source Todo slots; no Host delivery or new action authority is implied.
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[path = "common/face.rs"]
mod common;

fn selector(subject: &str, role: &str, action: &str, intent: &str) -> ComponentSubjectSelector {
    ComponentSubjectSelector::bound(
        BoundedSequence::try_from_iter([ComponentActionBinding::new(
            BoundedSequence::new(),
            action.into(),
            intent.into(),
        )
        .unwrap()])
        .unwrap(),
        BoundedSequence::try_from_iter([ComponentPropertyBinding::new(
            "flag".into(),
            "complete".into(),
            "".into(),
        )
        .unwrap()])
        .unwrap(),
        ComponentBindingRole::role(role.into()).unwrap(),
        subject.into(),
    )
    .unwrap()
}
fn correct() -> ComponentSubjectSelector {
    selector(
        "todo/item/task-1",
        "Item",
        "todo.complete.task-1",
        "todo/complete@1",
    )
}
fn recipe(selector: ComponentSubjectSelector) -> conduit_core::StructuredInfoValue {
    ComponentRecipeInfo::new(
        12,
        "test/required-item@1".into(),
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
fn refusal(face: &Face, selector: ComponentSubjectSelector) -> ComponentRecipeRefusal {
    prepare_component_recipe(recipe(selector), face)
        .err()
        .expect("must refuse")
}

#[test]
fn actual_todo_slot_preserves_original_truth_and_generic_fallback() {
    let mut face = common::face(2, 3, true);
    face.subjects.push(FaceSubject {
        identity: "open-domain".into(),
        role: FaceRole::Semantic(conduit_core::kind_id("biology/cell@1")),
        name: "An unknown valid domain".into(),
    });
    let face = common::rehash(face);
    let before = face.clone();
    let info = recipe(correct());
    assert!(info.canonical_bytes().unwrap().len() <= 65_536);
    let prepared = prepare_component_recipe(info, &face).unwrap();
    let item = face
        .subjects
        .iter()
        .position(|s| s.identity == "todo/item/task-1")
        .unwrap();
    assert_eq!(prepared.subject_regions[item], Some(0));
    assert_eq!(prepared.subject_regions.last(), Some(&None));
    assert_eq!(prepared.reading, plan_face_utterances(&face).unwrap());
    assert_eq!(face, before);
    // The existing current action and its source availability remain intact.
    assert_eq!(
        face.resolve_action(face.revision, "todo.complete.task-1")
            .unwrap()
            .intent,
        "todo/complete@1"
    );
}

#[test]
fn missing_wrong_and_forged_slots_refuse_before_rendering() {
    let face = common::face(2, 3, true);
    assert_eq!(
        refusal(
            &face,
            selector("missing", "Item", "todo.complete.task-1", "todo/complete@1")
        ),
        ComponentRecipeRefusal::MissingSlot
    );
    assert_eq!(
        refusal(
            &face,
            selector(
                "todo/item/task-1",
                "Collection",
                "todo.complete.task-1",
                "todo/complete@1"
            )
        ),
        ComponentRecipeRefusal::SlotRole
    );
    assert_eq!(
        refusal(
            &face,
            selector(
                "todo/item/task-1",
                "Item",
                "todo.complete.task-2",
                "todo/complete@1"
            )
        ),
        ComponentRecipeRefusal::SlotAction
    );
    assert_eq!(
        refusal(
            &face,
            selector("todo/item/task-1", "Item", "invented", "todo/complete@1")
        ),
        ComponentRecipeRefusal::SlotAction
    );
    assert_eq!(
        refusal(
            &face,
            selector(
                "todo/item/task-1",
                "Item",
                "todo.complete.task-1",
                "todo/reopen@1"
            )
        ),
        ComponentRecipeRefusal::SlotAction
    );
    let mut missing = face.clone();
    missing
        .properties
        .retain(|p| p.subject != "todo/item/task-1" || p.name != "complete");
    assert_eq!(
        refusal(&common::rehash(missing), correct()),
        ComponentRecipeRefusal::SlotProperty
    );
    let mut wrong = face.clone();
    wrong
        .properties
        .iter_mut()
        .find(|p| p.subject == "todo/item/task-1" && p.name == "complete")
        .unwrap()
        .value = FacePropertyValue::Text("false".into());
    assert_eq!(
        refusal(&common::rehash(wrong), correct()),
        ComponentRecipeRefusal::SlotProperty
    );
}

#[test]
fn unavailable_source_action_is_readable_but_gains_no_authority() {
    let face = common::face(1, 1, false);
    let prepared = prepare_component_recipe(recipe(correct()), &face).unwrap();
    assert_eq!(prepared.reading, plan_face_utterances(&face).unwrap());
    assert!(matches!(
        face.resolve_action(face.revision, "todo.complete.task-1"),
        Err(FaceActionRefusal::Unavailable { .. })
    ));
}

#[test]
fn duplicate_and_contradictory_source_facts_cannot_satisfy_a_slot() {
    let mut face = common::face(1, 1, true);
    let property = face
        .properties
        .iter()
        .find(|p| p.subject == "todo/item/task-1" && p.name == "complete")
        .unwrap()
        .clone();
    face.properties.push(property);
    assert_eq!(
        refusal(&common::rehash(face), correct()),
        ComponentRecipeRefusal::SlotProperty
    );
}

#[test]
fn authored_component_fore_checks_and_its_literal_binds_the_actual_todo_face() {
    use conduit_plot::*;
    let mut profiles = ProfileCatalog::new();
    profiles.insert_kind(component_mask_kind(65536)).unwrap();
    let mut startup = profiles.startup_catalog().unwrap();
    install_mask_plot_value_aliases(&mut startup).unwrap();
    for (name, schema) in [
        ("ComponentRecipeInfo", component_recipe_type()),
        ("VisualExpressionInfo", visual_expression_type()),
    ] {
        let profile = schema.profile().unwrap().value_kind().clone();
        startup
            .insert_structured_type(name, schema.clone())
            .unwrap();
        startup
            .ensure_structured_type(profile.as_str(), schema)
            .unwrap();
    }
    let source = include_str!("../../../../semantics/face/components/todo-required-item.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let component = checked
        .plots
        .iter()
        .find(|p| p.name == "todo-required-item")
        .unwrap();
    let binding = component.gears[0]
        .startup_bindings
        .iter()
        .find(|b| b.name == "recipe")
        .unwrap();
    let CanonicalStartupValue::Structured(value) = &binding.value else {
        panic!("checked recipe literal")
    };
    let prepared =
        prepare_component_recipe(value.try_concrete().unwrap(), &common::face(1, 1, true)).unwrap();
    assert!(prepared.subject_regions.contains(&Some(0)));
}

#[test]
fn broad_and_open_semantic_roles_are_distinct_typed_slots() {
    let mut face = common::face(1, 1, true);
    let role = "example/item@1";
    face.subjects
        .iter_mut()
        .find(|s| s.identity == "todo/item/task-1")
        .unwrap()
        .role = FaceRole::Semantic(conduit_core::kind_id(role));
    let face = common::rehash(face);
    assert_eq!(
        refusal(
            &face,
            selector(
                "todo/item/task-1",
                role,
                "todo.complete.task-1",
                "todo/complete@1"
            )
        ),
        ComponentRecipeRefusal::SlotRole
    );
    let ComponentSubjectSelector::Bound(binding) = correct() else {
        unreachable!()
    };
    let semantic = ComponentSubjectSelector::bound(
        binding.actions().clone(),
        binding.properties().clone(),
        ComponentBindingRole::semantic(role.into()).unwrap(),
        binding.subject().clone(),
    )
    .unwrap();
    prepare_component_recipe(recipe(semantic), &face).unwrap();
}

#[test]
fn changed_source_action_uses_generic_reading_instead_of_inventing_a_toggle() {
    let completed = common::face(0, 1, true);
    assert_eq!(
        refusal(&completed, correct()),
        ComponentRecipeRefusal::SlotAction
    );
    assert!(completed
        .resolve_action(completed.revision, "todo.reopen.task-1")
        .is_ok());
    let linear = render_linear_face(&completed).unwrap();
    assert!(linear.lines.join("\n").contains("reopen"));
}
