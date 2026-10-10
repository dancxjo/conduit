use super::*;
use conduit_core::{
    ExactDecimalQuantity, QuantityUnit, EXACT_DECIMAL_QUANTITY_ENCODED_LEN,
    EXACT_DECIMAL_QUANTITY_INFO_ID,
};
fn basis() -> PresentationContributionBasis {
    PresentationContributionBasis {
        checked_plot_id: "checked/thermostat".into(),
        plan_id: "plan/thermostat".into(),
        active_play_id: "play/thermostat".into(),
        required_interaction_context: None,
    }
}
#[test]
fn exact_celsius_content_retains_half_degrees_and_negative_observations() {
    let state = ThermostatState::default()
        .apply(Command::SetTarget(225))
        .unwrap()
        .apply(Command::Observe(Some(-35)))
        .unwrap();
    let face = fragment(&state, basis(), true).unwrap();
    for (subject, coordinate) in [("thermostat/target", 225), ("thermostat/current", -35)] {
        let property = face
            .properties
            .iter()
            .find(|property| property.subject == subject && property.name == "value")
            .unwrap();
        let PresentationPropertyValue::TypedValue { contract, bytes } = &property.value else {
            panic!("temperature must be exact typed content")
        };
        assert_eq!(contract.value_kind.as_str(), EXACT_DECIMAL_QUANTITY_INFO_ID);
        assert_eq!(
            contract.maximum_bytes,
            EXACT_DECIMAL_QUANTITY_ENCODED_LEN as u32
        );
        contract.validate(bytes).unwrap();
        let quantity = ExactDecimalQuantity::decode(bytes).unwrap();
        assert_eq!(quantity.unit(), QuantityUnit::Celsius);
        assert_eq!(
            (quantity.coefficient(), quantity.exponent()),
            (coordinate, -1)
        );
    }
    let missing = fragment(&ThermostatState::default(), basis(), true).unwrap();
    assert!(!missing
        .properties
        .iter()
        .any(|property| property.subject == "thermostat/current"
            && matches!(property.value, PresentationPropertyValue::TypedValue { .. })));
    assert!(!face
        .properties
        .iter()
        .any(|property| property.name == "target-decicelsius"));
    assert!(face
        .text
        .iter()
        .any(|text| text.subject == "thermostat/current" && text.text == "Observed temperature"));
}
#[test]
fn controls_expose_named_groups_and_selection_without_identity_prefix_inference() {
    let state = ThermostatState::default()
        .apply(Command::SetMode(Mode::Cool))
        .unwrap()
        .apply(Command::SetFan(Fan::On))
        .unwrap()
        .apply(Command::SetPreset(Preset::Eco))
        .unwrap();
    let face = fragment(&state, basis(), true).unwrap();
    for (group_name, selected_name) in [("Mode", "Cool"), ("Fan", "On"), ("Preset", "Eco")] {
        let group = face
            .subjects
            .iter()
            .find(|subject| subject.name == group_name)
            .unwrap();
        assert_eq!(
            group.role,
            PresentationRole::Semantic(conduit_core::kind_id(CHOICE_GROUP_ROLE))
        );
        let options = face
            .relationships
            .iter()
            .filter(|relationship| {
                relationship.source == group.identity
                    && relationship.kind == PresentationRelationshipKind::Contains
            })
            .map(|relationship| &relationship.target)
            .collect::<Vec<_>>();
        let selected = options
            .iter()
            .filter(|option| {
                face.properties.iter().any(|property| {
                    &property.subject == **option
                        && property.name == "selected"
                        && property.value == PresentationPropertyValue::Flag(true)
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert!(face
            .subjects
            .iter()
            .any(|subject| &subject.identity == *selected[0] && subject.name == selected_name));
        assert!(options
            .iter()
            .all(|option| face.actions.iter().any(|action| &action.target == *option)));
    }
}
#[test]
fn temperature_content_keeps_exact_source_basis_and_canonical_whole_degrees() {
    let original = basis();
    let face = fragment(&ThermostatState::default(), original.clone(), true).unwrap();
    assert_eq!(face.basis, original);
    let property = face
        .properties
        .iter()
        .find(|property| property.subject == "thermostat/target" && property.name == "value")
        .unwrap();
    let PresentationPropertyValue::TypedValue { bytes, .. } = &property.value else {
        panic!("typed temperature")
    };
    let quantity = ExactDecimalQuantity::decode(bytes).unwrap();
    assert_eq!((quantity.coefficient(), quantity.exponent()), (21, 0));
}
