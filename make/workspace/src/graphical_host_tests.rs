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
