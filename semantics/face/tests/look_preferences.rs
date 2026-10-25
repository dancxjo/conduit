use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;
fn preferences(look: Option<LookInfo>, unit: Option<LookTemperatureUnit>) -> LookPreferencesInfo {
    LookPreferencesInfo::new(
        look.map(|look| LookPreferenceLookOption::from_look(look).unwrap())
            .unwrap_or(LookPreferenceLookOption::None),
        unit.map(|unit| LookPreferenceTemperatureOption::Some(unit.into()))
            .unwrap_or(LookPreferenceTemperatureOption::None),
        1,
    )
    .unwrap()
}
fn with_unit(look: LookInfo, unit: LookTemperatureUnit) -> LookInfo {
    LookInfo::new(
        look.generative().clone(),
        look.identity().clone(),
        LookQuantityFacetOption::some(unit).unwrap(),
        look.spoken().clone(),
        *look.version(),
        look.visual().clone(),
    )
    .unwrap()
}
fn unit(look: &LookInfo) -> Option<LookTemperatureUnit> {
    match look.quantity() {
        LookQuantityFacetOption::Some(facet) => Some(*facet.temperature_unit()),
        _ => None,
    }
}
#[test]
fn palette_changes_inherit_the_retained_unit_and_explicit_encounter_unit_wins() {
    use LookPreferenceOrigin as O;
    use LookTemperatureUnit as U;
    let authored = with_unit(editorial_look().unwrap(), U::Celsius);
    let original = authored.clone();
    let retained = preferences(None, Some(U::Fahrenheit));
    let palette = preferences(Some(engineering_look().unwrap()), None);
    let resolved = resolve_look_preferences(&authored, Some(&retained), Some(&palette)).unwrap();
    assert_eq!(resolved.requested().identity(), ENGINEERING_LOOK_ID);
    assert_eq!(unit(resolved.requested()), Some(U::Fahrenheit));
    assert_eq!(resolved.look_origin(), O::EncounterLook);
    assert_eq!(resolved.quantity_origin(), O::RetainedTemperatureUnit);
    let encounter = preferences(Some(engineering_look().unwrap()), Some(U::Kelvin));
    let resolved = resolve_look_preferences(&authored, Some(&retained), Some(&encounter)).unwrap();
    assert_eq!(resolved.requested().identity(), ENGINEERING_LOOK_ID);
    assert_eq!(unit(resolved.requested()), Some(U::Kelvin));
    assert_eq!(resolved.quantity_origin(), O::EncounterTemperatureUnit);
    assert_eq!(authored, original);
}
#[test]
fn each_layer_has_explicit_look_then_unit_precedence() {
    use LookTemperatureUnit as U;
    let authored = with_unit(editorial_look().unwrap(), U::Celsius);
    let retained_look = with_unit(engineering_look().unwrap(), U::Kelvin);
    let retained = preferences(Some(retained_look), Some(U::Fahrenheit));
    let explicit = preferences(Some(with_unit(phosphor_look().unwrap(), U::Celsius)), None);
    let inherited = resolve_look_preferences(&authored, Some(&retained), None).unwrap();
    assert_eq!(unit(inherited.requested()), Some(U::Fahrenheit));
    assert_eq!(inherited.look_origin(), LookPreferenceOrigin::RetainedLook);
    let selected = resolve_look_preferences(&authored, Some(&retained), Some(&explicit)).unwrap();
    assert_eq!(unit(selected.requested()), Some(U::Celsius));
    assert_eq!(
        selected.quantity_origin(),
        LookPreferenceOrigin::EncounterLook
    );
}
#[test]
fn absence_and_removing_a_retained_override_return_to_authored_policy() {
    let authored = editorial_look().unwrap();
    let empty = preferences(None, None);
    let original = resolve_look_preferences(&authored, Some(&empty), None).unwrap();
    assert_eq!(original.requested(), &authored);
    assert_eq!(original.quantity_origin(), LookPreferenceOrigin::Absent);
    let celsius = with_unit(authored, LookTemperatureUnit::Celsius);
    let restored = resolve_look_preferences(&celsius, Some(&empty), None).unwrap();
    assert_eq!(restored.requested(), &celsius);
    assert_eq!(restored.quantity_origin(), LookPreferenceOrigin::Authored);
}
#[test]
fn selected_unit_survives_subsequent_visual_accessibility_fallback() {
    let retained = preferences(None, Some(LookTemperatureUnit::Kelvin));
    let base = editorial_look().unwrap();
    let authored = LookInfo::new(
        base.generative().clone(),
        base.identity().clone(),
        base.quantity().clone(),
        base.spoken().clone(),
        3,
        LookVisualFacetOption::None,
    )
    .unwrap();
    let resolved = resolve_look_preferences(&authored, Some(&retained), None).unwrap();
    let admitted = admit_visual_look(
        resolved.requested(),
        VisualUserConstraints {
            forced_colors: true,
            reduced_motion: true,
            minimum_text_scale_percent: 200,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    assert!(matches!(
        admitted.visual.outcome,
        VisualExpressionOutcome::PhosphorFallback { .. }
    ));
    assert_eq!(unit(&admitted.selected), Some(LookTemperatureUnit::Kelvin));
    assert_eq!(
        resolved.quantity_origin(),
        LookPreferenceOrigin::RetainedTemperatureUnit
    );
}
#[test]
fn preference_content_is_canonical_bounded_and_does_not_accept_a_foreign_or_forged_look() {
    let value = preferences(
        Some(editorial_look().unwrap()),
        Some(LookTemperatureUnit::Fahrenheit),
    );
    let bytes = encode_look_preferences(&value).unwrap();
    assert_eq!(decode_look_preferences(&bytes).unwrap(), value);
    assert!(bytes.len() < MAX_LOOK_PREFERENCES_BYTES);
    assert!(decode_look_preferences(&bytes[..bytes.len() - 1]).is_err());
    assert!(decode_look_preferences(&phosphor_look().unwrap().encode().unwrap()).is_err());
    assert!(matches!(
        decode_look_preferences(&vec![0; MAX_LOOK_PREFERENCES_BYTES + 1]),
        Err(LookPreferencesRefusal::TooLarge)
    ));
    let needle = EDITORIAL_LOOK_ID.as_bytes();
    let index = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap();
    let mut forged = bytes;
    forged[index + "conduit.look/editoria".len()] = b'X';
    assert!(
        decode_look_preferences(&forged).is_err(),
        "nested Look identity must still match its visual expression"
    );
    assert!(
        LookPreferencesInfo::new(
            LookPreferenceLookOption::None,
            LookPreferenceTemperatureOption::None,
            2
        )
        .is_err()
    );
}
