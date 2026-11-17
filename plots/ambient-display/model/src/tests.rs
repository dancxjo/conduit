use crate::{AmbientFaceRefusal, PreparedAmbientFaces};
use alloc::vec::Vec;
use conduit_face::FacePropertyValue;

use crate::test_fixtures::*;

#[test]
fn home_plot_has_one_bounded_contributor_and_no_embedded_realization() {
    let source = include_str!("../../../ambient-home/main.conduit");
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    crate::install_ambient_home_contributor(&mut startup, &mut profiles).unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "ambient-home", &profiles)
            .unwrap();
    let unnamed = source.replace("face/ambient-home(\"Home\")", "face/ambient-home");
    assert!(
        conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&unnamed),
            &startup,
        )
        .is_err(),
        "Home must retain the ordinary explicit Face label contract"
    );
    assert_eq!(expanded.expanded.gears.len(), 1);
    assert_eq!(expanded.input_bindings.len(), 1);
    assert!(expanded.output_bindings.is_empty());
    let kind = crate::ambient_home_contributor_kind().unwrap();
    assert_eq!(kind.inputs.len(), 1);
    assert_eq!(
        kind.inputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert_eq!(kind.limits.max_queue_items, 1);
    assert_eq!(kind.limits.max_queue_bytes, 8192);
    for forbidden in ["http://", "https://", "motherbrain", "threebee", "Seattle"] {
        assert!(!source.contains(forbidden));
    }
}

#[test]
fn bundled_observation_reuses_both_exact_domain_contracts() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let picture = observed_wallpaper(&prepared, "available", 301_000);
    let current = clock(&prepared, "fresh", 150, 150);
    let observation = record(&prepared.home_type().value_type, |name, _| match name {
        "wallpaper" => picture.clone(),
        "clock_weather" => current.clone(),
        _ => panic!("Home field"),
    });
    let encoded = observation.canonical_bytes().unwrap();
    assert!(
        encoded.len() <= crate::AMBIENT_HOME_MAXIMUM_BYTES as usize,
        "Home observation is {} bytes, beyond its admitted Fore",
        encoded.len()
    );
    let fragment = prepared.home_observation(basis(), &observation).unwrap();
    assert_eq!(
        prepared
            .home_canonical_observation(basis(), &encoded)
            .unwrap(),
        fragment
    );
    assert!(matches!(
        prepared.home_canonical_observation(basis(), b"not a canonical observation"),
        Err(AmbientFaceRefusal::InvalidObservation)
    ));
    assert!(matches!(
        prepared.home_canonical_observation(
            basis(),
            &alloc::vec![0; crate::AMBIENT_HOME_MAXIMUM_BYTES as usize + 1]
        ),
        Err(AmbientFaceRefusal::InvalidObservation)
    ));
    assert!(matches!(
        prepared.home_canonical_observation(basis(), &current.canonical_bytes().unwrap()),
        Err(AmbientFaceRefusal::WrongType)
    ));
    assert_eq!(fragment.subjects[0].name, "Home");
    assert!(matches!(
        prepared.home_observation(basis(), &current),
        Err(AmbientFaceRefusal::WrongType)
    ));
}

#[test]
fn home_is_one_contribution_with_both_domains_and_stable_subjects() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let picture = observed_wallpaper(&prepared, "available", 301_000);
    let current = clock(&prepared, "fresh", 150, 150);
    let first = prepared.home(basis(), &picture, &current).unwrap();
    assert_eq!(first.basis, basis());
    assert_eq!(first.subjects[0].name, "Home");
    assert_eq!(first.composition.len(), 2);
    assert_eq!(
        first.composition[0].kind,
        conduit_face::FaceCompositionKind::Subordinate
    );
    assert_eq!(
        first.composition[1].kind,
        conduit_face::FaceCompositionKind::Associate
    );
    assert!(first
        .composition
        .iter()
        .all(|relation| relation.target == first.subjects[0].identity));
    let subjects = first.subjects.clone();
    let displayed = face(first);
    let linear = conduit_face::render_linear_face(&displayed)
        .unwrap()
        .lines
        .join("\n");
    for required in ["Mountain light", "Seattle", "°C"] {
        assert!(linear.contains(required), "Home missing {required}");
    }
    let updated = clock(&prepared, "stale", 201, 201);
    let second = prepared.home(basis(), &picture, &updated).unwrap();
    assert_eq!(subjects, second.subjects);
}

#[test]
fn wallpaper_failure_preserves_subjects_without_inventing_image_content() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let mut identities = None;
    for state in ["available", "stale", "unavailable"] {
        let fragment = prepared
            .wallpaper_observation(basis(), &observed_wallpaper(&prepared, state, 301_000))
            .unwrap();
        let current: Vec<_> = fragment
            .subjects
            .iter()
            .map(|subject| subject.identity.clone())
            .collect();
        if let Some(expected) = &identities {
            assert_eq!(&current, expected);
        } else {
            identities = Some(current);
        }
        assert!(fragment
            .properties
            .iter()
            .any(|property| property.name == "availability"
                && property.value == FacePropertyValue::Text(state.into())));
        let has_content = fragment
            .properties
            .iter()
            .any(|property| matches!(property.value, FacePropertyValue::Content(_)));
        assert_eq!(has_content, state != "unavailable");
        let displayed = face(fragment);
        let linear = conduit_face::render_linear_face(&displayed)
            .unwrap()
            .lines
            .join("\n");
        if state != "available" {
            assert!(linear.contains("Acquisition failed"));
        }
    }
}

#[test]
fn wallpaper_presents_credit_cadence_and_exact_content_without_acquiring_it() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let fragment = prepared
        .wallpaper(basis(), &wallpaper(&prepared, 301_000))
        .unwrap();
    assert_eq!(fragment.basis, basis());
    assert!(fragment.actions.is_empty());
    let face = face(fragment);
    let linear = conduit_face::render_linear_face(&face)
        .unwrap()
        .lines
        .join("\n");
    let spoken = conduit_face::plan_face_utterances(&face)
        .unwrap()
        .clauses
        .iter()
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for required in [
        "Mountain light",
        "Sunrise over the Cascades",
        "Example photographer",
        "License: CC BY-SA 4.0",
        "CC BY-SA 4.0",
        "Rotation interval: 5 minutes",
    ] {
        assert!(linear.contains(required), "linear missing {required}");
        assert!(spoken.contains(required), "spoken missing {required}");
    }
    let content = face
        .properties
        .iter()
        .find(|property| property.name == "image-content")
        .unwrap();
    assert_eq!(content.value, FacePropertyValue::Content(resource()));
    assert!(face
        .properties
        .iter()
        .any(|property| property.name == "image"
            && matches!(property.value, FacePropertyValue::TypedValue { .. })));
    assert!(face
        .properties
        .iter()
        .any(|property| property.name == "next-rotation-unix-ms"
            && property.value == FacePropertyValue::Count(301_000)));
}

#[test]
fn clock_and_weather_are_meaningful_in_linear_and_spoken_masks() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let fragment = prepared
        .clock_weather(basis(), &clock(&prepared, "fresh", 150, 150))
        .unwrap();
    let face = face(fragment);
    let linear = conduit_face::render_linear_face(&face)
        .unwrap()
        .lines
        .join("\n");
    let spoken = conduit_face::plan_face_utterances(&face)
        .unwrap()
        .clauses
        .iter()
        .map(|clause| clause.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for required in [
        "2026-11-15",
        "21:30:15",
        "America/Los_Angeles",
        "Seattle",
        "Light rain",
        "Open-Meteo",
    ] {
        assert!(linear.contains(required), "linear missing {required}");
        assert!(spoken.contains(required), "spoken missing {required}");
    }
    assert!(spoken.contains("-2.125 degrees Celsius"));
    assert!(linear.contains("iana-test/rules"));
    assert!(face
        .properties
        .iter()
        .any(|property| property.name == "availability"
            && property.value == FacePropertyValue::Text("fresh".into())));
    assert!(face
        .properties
        .iter()
        .any(|property| property.name == "usable-until-unix-ms"
            && property.value == FacePropertyValue::Count(200)));
}

#[test]
fn stale_and_unavailable_weather_preserve_clock_without_claiming_current_conditions() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    for (availability, snapshot, expected) in [
        ("stale", 201, "Weather is out of date"),
        (
            "unavailable",
            150,
            "Weather unavailable: Provider could not be reached",
        ),
    ] {
        let fragment = prepared
            .clock_weather(basis(), &clock(&prepared, availability, snapshot, snapshot))
            .unwrap();
        let rendered = conduit_face::plan_face_utterances(&face(fragment))
            .unwrap()
            .clauses
            .iter()
            .map(|clause| clause.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("21:30:15"));
        assert!(rendered.contains(expected));
        if availability == "unavailable" {
            assert!(!rendered.contains("Light rain"));
        }
    }
}

#[test]
fn projections_refuse_wrong_types_law_failures_and_invalid_basis() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let observation = clock(&prepared, "fresh", 150, 150);
    assert_eq!(
        prepared.wallpaper(basis(), &observation).unwrap_err(),
        AmbientFaceRefusal::WrongType
    );
    assert_eq!(
        prepared
            .wallpaper(basis(), &wallpaper(&prepared, 300_999))
            .unwrap_err(),
        AmbientFaceRefusal::InvalidObservation
    );
    assert_eq!(
        prepared
            .clock_weather(basis(), &clock(&prepared, "fresh", 201, 150))
            .unwrap_err(),
        AmbientFaceRefusal::InvalidObservation
    );
    let mut invalid = basis();
    invalid.plan_id = "".into();
    assert_eq!(
        prepared.clock_weather(invalid, &observation).unwrap_err(),
        AmbientFaceRefusal::InvalidBasis
    );
}

#[test]
fn home_projection_envelope_covers_all_availability_combinations() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let bound = crate::ambient_home_fragment_backing_bound();
    assert!(prepared.projection_workspace_bytes().unwrap() > bound);
    for wallpaper in ["available", "stale", "unavailable"] {
        for weather in ["fresh", "stale", "unavailable"] {
            let now = if weather == "stale" { 201 } else { 150 };
            let picture = observed_wallpaper(&prepared, wallpaper, 301_000);
            let clock = clock(&prepared, weather, now, now);
            let fragment = prepared.home(basis(), &picture, &clock).unwrap();
            let actual = crate::ambient_fragment_backing_bytes(&fragment).unwrap();
            assert!(
                actual <= bound,
                "{wallpaper}/{weather}: {actual} exceeds {bound}"
            );
            assert_eq!(fragment.subjects.capacity(), 8);
            assert_eq!(fragment.properties.capacity(), 32);
        }
    }
}
