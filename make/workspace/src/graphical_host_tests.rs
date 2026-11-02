//! Construction intent only: no display is opened and no Ready offer is minted.
use conduit_host_make::{check_host_configuration, parse_host_configuration_conduit};

const PI3: &str = r#"host display-host {
  schema = 1
  target = {architecture: "aarch64", machine: "raspberry-pi-3-model-b-plus-rev-1.3", board: "raspberry-pi-3-model-b-plus-rev-1.3", os: "linux"}
  base = {kind: "display/scanout", implementation: "display/linear-framebuffer@1"}
  need = {id: "display/main", class: "face/surface", slots: 1, bytes: 2457600}
  limits = {static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}
}"#;

#[test]
fn pi3_graphical_construction_requires_explicit_scanout_and_surface() {
    let catalog = super::catalog();
    let packages = super::package_set();
    let check = |source: &str| {
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &catalog,
            &packages,
        )
    };
    let checked = check(PI3).expect("Pi3 closes its selected graphical construction");
    let profile = checked.profile();
    assert!(profile
        .presenters
        .iter()
        .any(|presenter| { presenter.implementation == "presenter/native-graphical@1" }));
    assert!(profile
        .facilities
        .iter()
        .any(|facility| facility == "compositor/native@1"));
    assert!(profile
        .host_calls
        .iter()
        .any(|call| call == "conduit.host/present@1"));
    let zero_surface = PI3.replace("slots: 1", "slots: 0");
    assert!(
        check(&zero_surface).is_err(),
        "zero surface is not a graphical grant"
    );
    let foreign_driver = PI3.replace("display/linear-framebuffer@1", "browser/dom@1");
    assert!(
        check(&foreign_driver).is_err(),
        "unrelated driver cannot close scanout"
    );
}

#[test]
fn direct_display_initialization_is_explicit_finite_and_source_bound() {
    use conduit_linear_framebuffer_make::CheckedScanoutInitialization;
    let catalog = super::catalog();
    let packages = super::package_set();
    let check = |source: &str| {
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &catalog,
            &packages,
        )
    };
    let source = PI3.replace(
        "implementation: \"display/linear-framebuffer@1\"}",
        "implementation: \"display/linear-framebuffer@1\", initialization: {backend: \"KMSDRM\", width: 1024, height: 600}}",
    );
    let checked = check(&source).unwrap();
    let intent = CheckedScanoutInitialization::from_configuration(&checked).unwrap();
    assert_eq!(intent.configuration_id(), checked.configuration_id());
    assert_eq!((intent.width(), intent.height()), (1024, 600));
    assert_eq!(intent.surface_bytes(), 2_457_600);
    let changed = check(&source.replace("width: 1024", "width: 1280")).unwrap();
    assert_ne!(checked.configuration_id(), changed.configuration_id());
    let canonical =
        conduit_host_make::canonical_host_configuration_conduit(checked.configuration()).unwrap();
    assert_eq!(
        check(&canonical).unwrap().configuration_id(),
        checked.configuration_id()
    );
    for invalid in [
        source.replace("KMSDRM", "dummy"),
        source.replace("width: 1024, ", ""),
        source.replace("width: 1024", "width: 0"),
        source.replace("height: 600", "height: 4097"),
        source.replace("height: 600", "unknown: 600"),
    ] {
        assert!(check(&invalid).is_err());
    }
    // Independent dimension limits do not bypass the raster's total byte cap.
    let oversized = check(&source.replace("width: 1024", "width: 4096")).unwrap();
    assert!(CheckedScanoutInitialization::from_configuration(&oversized).is_err());
    let uninitialized = check(PI3).unwrap();
    assert!(CheckedScanoutInitialization::from_configuration(&uninitialized).is_err());
}
