#![cfg(feature = "plot-catalog")]

//! Preparation/checker proof only: no acquisition, clock, timer or renderer.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, RESOURCE_REFERENCE_INFO_ID,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CheckedNativeType, CheckedSyntaxDocument, PortableExpressionProgram, ProfileCatalog,
    StartupCatalog,
};

const TYPES: &str = include_str!("../../../plots/ambient-display/types.conduit");
const WALLPAPER: &str = include_str!("../../../plots/ambient-wallpaper/main.conduit");
const CLOCK_WEATHER: &str = include_str!("../../../plots/ambient-clock-weather/main.conduit");

fn checked_with(source: &str) -> CheckedSyntaxDocument {
    let mut startup = conduit_face::image_resource_source_catalog().unwrap();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_ambient_wallpaper_face_kind(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_ambient_clock_snapshot_face_kind(&mut startup, &mut profile)
        .unwrap();
    check_syntax_document(&parse_syntax_document(source), &startup).unwrap()
}

fn checked() -> CheckedSyntaxDocument {
    checked_with(&format!("{TYPES}\n{WALLPAPER}\n{CLOCK_WEATHER}"))
}

fn native<'a>(checked: &'a CheckedSyntaxDocument, name: &str) -> &'a CheckedNativeType {
    checked
        .native_types
        .iter()
        .find(|ty| ty.name == name)
        .unwrap()
}

#[test]
fn wallpaper_observation_is_bounded_and_provider_neutral() {
    let checked = checked();
    for name in ["ambient-wallpaper/current", "ambient-clock-weather/current"] {
        let expanded =
            expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
        assert_eq!(expanded.input_bindings.len(), 1);
        assert_eq!(expanded.output_bindings.len(), 1);
        assert_eq!(expanded.expanded.gears.len(), 1);
        assert_eq!(
            expanded.expanded.gears[0].kind_contract_revision.as_str(),
            conduit_plot::PURE_EXPRESSION_REVISION
        );
    }
    let image = native(&checked, "AmbientWallpaperImage");
    let StructuredInfoTypeShape::Record { fields, .. } = image.value_type.shape() else {
        panic!("image metadata must be a finite record")
    };
    let resource = fields.iter().find(|field| field.name() == "image").unwrap();
    assert_eq!(
        resource.value_type(),
        &conduit_semantic_catalog::image_resource_type()
    );
    let StructuredInfoTypeShape::Record { fields, .. } = resource.value_type().shape() else {
        panic!("image must retain the existing resource contract")
    };
    assert!(
        matches!(fields.iter().find(|field| field.name() == "content").unwrap().value_type().shape(),
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == RESOURCE_REFERENCE_INFO_ID)
    );
    let mut startup = StartupCatalog::new();
    conduit_semantic_catalog::install_ambient_display_types(&mut startup).unwrap();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_ambient_wallpaper_face_kind(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_ambient_clock_snapshot_face_kind(&mut startup, &mut profile)
        .unwrap();
    let consumer = check_syntax_document(&parse_syntax_document(WALLPAPER), &startup).unwrap();

    let expanded =
        expand_canonical_plot_for_authoring(&consumer, "ambient-wallpaper", &profile).unwrap();
    assert!(
        expanded
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str()
                == conduit_semantic_catalog::AMBIENT_WALLPAPER_FACE_KIND)
    );
    for source in [TYPES, WALLPAPER, CLOCK_WEATHER] {
        for binding in ["http://", "https://", "Seattle", "motherbrain", "threebee"] {
            assert!(!source.contains(binding), "authored binding: {binding}");
        }
    }
}

fn primitive<T: conduit_plot::rust_binding::NativePrimitive>(
    ty: &StructuredInfoType,
    value: T,
) -> StructuredInfoValue {
    conduit_plot::rust_binding::primitive_into_structured(ty.clone(), &value).unwrap()
}

fn record(
    ty: &StructuredInfoType,
    mut value: impl FnMut(&str, &StructuredInfoType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("expected record")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                StructuredFieldValue::new(field.name(), value(field.name(), field.value_type()))
                    .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn weather(
    ty: &CheckedNativeType,
    reference: u64,
    observed: u64,
    expiry: u64,
    temperature: i64,
    conditions: &str,
) -> StructuredInfoValue {
    record(&ty.value_type, |name, field_type| match name {
        "reference_at_unix_ms" => primitive(field_type, reference),
        "reading" => record(field_type, |name, field_type| match name {
            "location" => primitive(field_type, "Example place".to_owned()),
            "attribution" => primitive(field_type, "Observation source".to_owned()),
            "conditions" => primitive(field_type, conditions.to_owned()),
            "temperature_celsius_milli" => primitive(field_type, temperature),
            "observed_at_unix_ms" => primitive(field_type, observed),
            "usable_until_unix_ms" => primitive(field_type, expiry),
            _ => panic!("unknown reading field"),
        }),
        _ => panic!("unknown freshness field"),
    })
}

fn valid(ty: &CheckedNativeType, value: &StructuredInfoValue) -> bool {
    conduit_plot::rust_binding::validate_native_contracts(value, &ty.value_contracts).is_ok()
        && conduit_plot::rust_binding::validate_native_invariants(value, &ty.invariants).is_ok()
}

#[test]
fn weather_freshness_boundaries_and_observation_bounds_remain_explicit() {
    let checked = checked();
    let fresh = native(&checked, "AmbientFreshWeather");
    let stale = native(&checked, "AmbientStaleWeather");
    assert!(valid(fresh, &weather(fresh, 100, 100, 200, -2_000, "Snow")));
    assert!(valid(fresh, &weather(fresh, 200, 100, 200, -2_000, "Snow")));
    assert!(!valid(
        fresh,
        &weather(fresh, 201, 100, 200, -2_000, "Snow")
    ));
    assert!(!valid(fresh, &weather(fresh, 99, 100, 200, -2_000, "Snow")));
    assert!(valid(stale, &weather(stale, 201, 100, 200, -2_000, "Snow")));
    assert!(!valid(
        stale,
        &weather(stale, 200, 100, 200, -2_000, "Snow")
    ));
    assert!(!valid(
        fresh,
        &weather(fresh, 150, 100, 200, 100_001, "Hot")
    ));
    assert!(!valid(
        fresh,
        &weather(fresh, 150, 100, 200, 20_000, &"a".repeat(81))
    ));
}

fn clock_weather(
    checked: &CheckedSyntaxDocument,
    reference: u64,
    snapshot: u64,
    availability: &str,
) -> StructuredInfoValue {
    record(
        &native(checked, "AmbientClockWeather").value_type,
        |name, field_type| match name {
            "local_date" => primitive(field_type, "2026-11-15".to_owned()),
            "local_time" => primitive(field_type, "12:00:00".to_owned()),
            "timezone" => primitive(field_type, "Example/Local".to_owned()),
            "timezone_rule_set" => primitive(field_type, "example/rules@1".to_owned()),
            "utc_offset_minutes" => primitive(field_type, 0_i64),
            "observed_at_unix_ms" => primitive(field_type, snapshot),
            "weather" => {
                let StructuredInfoTypeShape::Variant { cases, .. } = field_type.shape() else {
                    panic!("weather availability is a closed variant")
                };
                let case = cases
                    .iter()
                    .find(|case| case.tag() == availability)
                    .unwrap();
                let payload = match availability {
                    "fresh" => weather(
                        native(checked, "AmbientFreshWeather"),
                        reference,
                        100,
                        200,
                        20_000,
                        "Clear",
                    ),
                    "stale" => weather(
                        native(checked, "AmbientStaleWeather"),
                        reference,
                        100,
                        200,
                        20_000,
                        "Clear",
                    ),
                    "unavailable" => primitive(case.payload_type(), "No observation".to_owned()),
                    _ => panic!("unknown weather availability"),
                };
                StructuredInfoValue::variant(field_type.clone(), availability, payload).unwrap()
            }
            _ => panic!("unknown clock/weather field"),
        },
    )
}

#[test]
fn weather_reference_must_match_the_current_clock_snapshot() {
    let checked = checked();
    let snapshot = native(&checked, "AmbientClockWeather");
    assert!(valid(snapshot, &clock_weather(&checked, 150, 150, "fresh")));
    assert!(!valid(
        snapshot,
        &clock_weather(&checked, 150, 250, "fresh")
    ));
    assert!(!valid(
        snapshot,
        &clock_weather(&checked, 150, 149, "fresh")
    ));
    assert!(valid(snapshot, &clock_weather(&checked, 201, 201, "stale")));
    assert!(!valid(
        snapshot,
        &clock_weather(&checked, 201, 250, "stale")
    ));
    assert!(valid(
        snapshot,
        &clock_weather(&checked, 0, 250, "unavailable")
    ));
}

#[test]
fn clock_weather_observation_keeps_freshness_and_locality() {
    let checked = checked();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "ambient-clock-weather/current",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(program) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("ordinary pure expression program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
    assert_eq!(
        program.input_type,
        native(&checked, "AmbientClockWeather").value_type
    );
    assert_eq!(program.output_type, program.input_type);
    let StructuredInfoTypeShape::Variant { cases, .. } =
        native(&checked, "AmbientWeather").value_type.shape()
    else {
        panic!("weather availability must be a closed variant")
    };
    assert_eq!(cases.len(), 3);
    let mut startup = StartupCatalog::new();
    conduit_semantic_catalog::install_ambient_display_types(&mut startup).unwrap();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_ambient_wallpaper_face_kind(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_ambient_clock_snapshot_face_kind(&mut startup, &mut profile)
        .unwrap();
    let consumer = check_syntax_document(&parse_syntax_document(CLOCK_WEATHER), &startup).unwrap();

    let expanded =
        expand_canonical_plot_for_authoring(&consumer, "ambient-clock-weather", &profile).unwrap();
    assert!(expanded
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str()
            == conduit_semantic_catalog::AMBIENT_CLOCK_SNAPSHOT_FACE_KIND));
}

fn empty_case(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("expected a finite variant")
    };
    let case = cases.iter().find(|case| case.tag() == tag).unwrap();
    let payload = primitive(case.payload_type(), ());
    StructuredInfoValue::variant(ty.clone(), tag, payload).unwrap()
}

fn photo(ty: &StructuredInfoType, id: u8) -> StructuredInfoValue {
    use conduit_core::{
        kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity,
    };
    let content = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([id; 32]),
        content_profile: kind_id("media/image-rgba8@1"),
        access_class: ResourceClassId::from("conduit.resource/image-content@1"),
        extent: ResourceExtent {
            bytes: 16,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([id + 10; 32]),
            expires_at: None,
        },
    }
    .encode()
    .unwrap();
    record(ty, |name, field_type| match name {
        "image" => record(field_type, |name, field_type| match name {
            "content" => StructuredInfoValue::leaf(field_type.clone(), content.clone()).unwrap(),
            "color_profile" => empty_case(field_type, "absent"),
            "format" => empty_case(field_type, "rgba8"),
            "extent" => record(field_type, |_, field_type| primitive(field_type, 2_u64)),
            "image_frame" => primitive(field_type, format!("photo/{id}")),
            _ => panic!("unknown image resource field"),
        }),
        "title" => primitive(field_type, format!("Photo {id}")),
        "caption" => primitive(field_type, format!("Scene {id}")),
        "attribution" => primitive(field_type, format!("Photographer {id}")),
        "license" => primitive(field_type, "CC BY-SA 4.0".to_owned()),
        "source" => primitive(field_type, format!("Collection {id}")),
        _ => panic!("unknown photo field"),
    })
}

fn four_photo_deck(checked: &CheckedSyntaxDocument) -> StructuredInfoValue {
    record(
        &native(checked, "AmbientWallpaperDeckFour").value_type,
        |name, ty| {
            photo(
                ty,
                match name {
                    "current" => 1,
                    "next" => 2,
                    "after" => 3,
                    "last" => 4,
                    _ => panic!("unknown deck slot"),
                },
            )
        },
    )
}

fn rotation_program(checked: &CheckedSyntaxDocument) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(
        checked,
        "ambient-wallpaper/rotate-four",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(program) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("ordinary pure expression program")
    };
    PortableExpressionProgram::from_canonical_hex(program).unwrap()
}

#[test]
fn four_photo_rotation_retains_exact_content_and_attribution() {
    let checked = checked();
    let program = rotation_program(&checked);
    let original = four_photo_deck(&checked);
    let original_bytes = original.canonical_bytes().unwrap();
    let mut current = original_bytes.clone();
    for rotation in 1..=4 {
        current = program.evaluate(&current).unwrap();
        let decoded = StructuredInfoValue::from_canonical_bytes(&current).unwrap();
        let expected = record(&program.output_type, |name, ty| {
            let index = match name {
                "current" => 0,
                "next" => 1,
                "after" => 2,
                "last" => 3,
                _ => panic!("unknown deck slot"),
            };
            photo(ty, (index + rotation) % 4 + 1)
        });
        assert_eq!(decoded, expected, "transition {rotation}");
    }
    assert_eq!(current, original_bytes);
}

#[test]
fn four_photo_rotation_refuses_missing_or_foreign_cards() {
    let checked = checked();
    let deck = four_photo_deck(&checked);
    let conduit_core::StructuredInfoValueShape::Record(fields) = deck.shape() else {
        panic!("deck is an exact finite record")
    };
    assert!(StructuredInfoValue::record(deck.value_type().clone(), fields[..3].to_vec()).is_err());
    let program = rotation_program(&checked);
    let foreign = photo(&native(&checked, "AmbientWallpaperImage").value_type, 1);
    assert!(program
        .evaluate(&foreign.canonical_bytes().unwrap())
        .is_err());
}
