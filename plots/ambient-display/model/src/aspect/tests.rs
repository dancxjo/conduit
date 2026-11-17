use super::*;
use crate::test_fixtures::*;

#[test]
fn independent_aspects_project_only_their_subjects_from_one_checked_observation() {
    let faces = PreparedAmbientFaces::prepare().unwrap();
    let wallpaper = observed_wallpaper(&faces, "available", 301_000);
    let current_clock = clock(&faces, "fresh", 150, 150);
    let observation = record(&faces.home_type().value_type, |name, _| match name {
        "wallpaper" => wallpaper.clone(),
        "clock_weather" => current_clock.clone(),
        _ => panic!("field"),
    });
    let bytes = observation.canonical_bytes().unwrap();
    for aspect in [
        AmbientDisplayAspect::Wallpaper,
        AmbientDisplayAspect::ClockWeather,
    ] {
        let expected = if aspect == AmbientDisplayAspect::Wallpaper {
            faces.wallpaper_observation(basis(), &wallpaper).unwrap()
        } else {
            faces.clock_weather(basis(), &current_clock).unwrap()
        };
        let projected = faces
            .aspect_canonical_observation(aspect, basis(), &bytes)
            .unwrap();
        assert_eq!(projected, expected);
        assert!(!projected.subjects.iter().any(|s| s.name == "Home"));
    }
    let bad_clock = clock(&faces, "fresh", 150, 151);
    let invalid = record(&faces.home_type().value_type, |name, _| match name {
        "wallpaper" => wallpaper.clone(),
        "clock_weather" => bad_clock.clone(),
        _ => panic!("field"),
    })
    .canonical_bytes()
    .unwrap();
    assert!(faces
        .aspect_canonical_observation(AmbientDisplayAspect::Wallpaper, basis(), &invalid)
        .is_err());
}

#[test]
fn authored_plots_expand_to_two_distinct_contributors() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    crate::install_ambient_home_contributor(&mut startup, &mut profiles).unwrap();
    crate::install_ambient_observe(&mut startup, &mut profiles).unwrap();
    let source = include_str!("../../../../ambient-home/aspects.conduit")
        .split_once("plot ambient-wallpaper-display")
        .unwrap()
        .1;
    let source = alloc::format!("plot ambient-wallpaper-display{source}");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "ambient-aspects", &profiles)
            .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 2);
    // Authored composition expands Plots; it does not invent checked Back offers.
    assert!(expanded.expanded.realization_backs.is_empty());
    assert_eq!(expanded.input_bindings.len(), 2);
    let kinds = expanded
        .expanded
        .gears
        .iter()
        .map(|g| g.kind_id.as_str())
        .collect::<alloc::vec::Vec<_>>();
    assert!(kinds.contains(&AmbientDisplayAspect::Wallpaper.kind()));
    assert!(kinds.contains(&AmbientDisplayAspect::ClockWeather.kind()));
    assert!(!kinds.contains(&AmbientDisplayAspect::Home.kind()));
}
