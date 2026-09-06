use super::*;
use alloc::vec;
use conduit_core::{
    GearId, StateContinuation, StructuredFieldType, StructuredFieldValue, StructuredInfoValue,
    BOOL_INFO_ID,
};
use conduit_form::{ProfileCatalog, StartupCatalog};

fn boolean() -> (StructuredInfoType, StructuredInfoValue) {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let initial = StructuredInfoValue::leaf(ty.clone(), b"false".to_vec()).unwrap();
    (ty, initial)
}

fn catalogs(
    ty: &StructuredInfoType,
    initial: &StructuredInfoValue,
) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    startup.insert_structured_type("Cell", ty.clone()).unwrap();
    install_state_value_kind("Cell", ty, initial, &mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn authored_initialization_has_an_exact_type_and_no_semantic_step_count() {
    let (ty, initial) = boolean();
    let (startup, profile) = catalogs(&ty, &initial);
    let form = conduit_form::parse_with_startup(
        "form retained {\n cell: state/value(initial = true)\n}\n",
        &startup,
        &profile,
    )
    .unwrap();
    let state = derive_state_boundary(&form, &GearId::from("retained/cell"), 64).unwrap();
    assert_eq!(state.value_kind, *ty.profile().unwrap().value_kind());
    assert_eq!(
        state.initial_value,
        StructuredInfoValue::leaf(ty, b"true".to_vec())
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    assert_ne!(state.initial_value, initial.canonical_bytes().unwrap());
    assert_eq!(state.continuation, StateContinuation::ExternallyBounded);
    assert_eq!(state.state_id.as_str(), state.gear_id.as_str());
    assert_eq!(state.maximum_value_bytes, 64);
    assert_eq!(
        derive_state_boundary(&form, &state.gear_id, 1),
        Err(StateValueAdmissionError::InitialValueExceedsCapacity)
    );
    assert_eq!(
        derive_state_boundary(&form, &state.gear_id, 0),
        Err(StateValueAdmissionError::InvalidCapacity)
    );
}

#[test]
fn the_same_kind_specializes_to_a_distinct_finite_record_profile() {
    let (leaf, initial) = boolean();
    let ty = StructuredInfoType::record(
        kind_id("test/switch@1"),
        vec![StructuredFieldType::new("on", leaf.clone()).unwrap()],
    )
    .unwrap();
    let initial = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new("on", initial).unwrap()],
    )
    .unwrap();
    let (startup, profile) = catalogs(&ty, &initial);
    let form = conduit_form::parse_with_startup(
        "form retained {\n cell: state/value(initial = {on: true})\n}\n",
        &startup,
        &profile,
    )
    .unwrap();
    let state = derive_state_boundary(&form, &GearId::from("retained/cell"), 128).unwrap();
    assert_eq!(state.value_kind, *ty.profile().unwrap().value_kind());
    assert_ne!(state.value_kind, *leaf.profile().unwrap().value_kind());
    assert_eq!(
        state.initial_value,
        StructuredInfoValue::record(
            ty,
            vec![StructuredFieldValue::new(
                "on",
                StructuredInfoValue::leaf(leaf, b"true".to_vec()).unwrap()
            )
            .unwrap(),]
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    );
}

#[test]
fn missing_wrong_typed_and_forged_initializations_do_not_become_state() {
    let (ty, initial) = boolean();
    let (startup, profile) = catalogs(&ty, &initial);
    for source in [
        "form retained {\n cell: state/value\n}\n",
        "form retained {\n cell: state/value(initial = 123)\n}\n",
    ] {
        assert!(conduit_form::parse_with_startup(source, &startup, &profile).is_err());
    }
    let mut form = conduit_form::parse_with_startup(
        "form retained {\n cell: state/value(initial = true)\n}\n",
        &startup,
        &profile,
    )
    .unwrap();
    form.checked_form_id = conduit_core::CheckedFormId::from("forged");
    assert_eq!(
        derive_state_boundary(&form, &GearId::from("retained/cell"), 64),
        Err(StateValueAdmissionError::InvalidForm)
    );
}

#[test]
fn a_matching_unary_face_does_not_authorize_state_initialization() {
    let (ty, initial) = boolean();
    let (mut startup, mut profile) = catalogs(&ty, &initial);
    let mut signature = startup.signature(STATE_VALUE_KIND).unwrap().clone();
    signature.kind = "fixture/unary".into();
    startup.insert(signature).unwrap();
    let mut definition = profile.get(&kind_id(STATE_VALUE_KIND)).unwrap().clone();
    definition.kind_id = kind_id("fixture/unary");
    profile.insert(definition).unwrap();
    let form = conduit_form::parse_with_startup(
        "form retained {\n cell: fixture/unary(initial = true)\n}\n",
        &startup,
        &profile,
    )
    .unwrap();
    assert_eq!(
        derive_state_boundary(&form, &GearId::from("retained/cell"), 64),
        Err(StateValueAdmissionError::WrongContract)
    );
}
