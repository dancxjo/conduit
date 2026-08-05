#[test]
fn dependency_and_profile_installation_drawbridge_is_explicit() {
    let runtime_manifest = include_str!("../Cargo.toml");
    let form_manifest = include_str!("../../conduit-form/Cargo.toml");
    let signal_manifest = include_str!("../../conduit-signal/Cargo.toml");
    let std_manifest = include_str!("../../../hosts/std/Cargo.toml");
    let std_host = include_str!("../../../hosts/std/src/lib.rs");

    assert!(!runtime_manifest.contains("conduit-signal"));
    assert!(!form_manifest.contains("conduit-signal"));
    assert!(signal_manifest.contains("default = []"));
    assert!(signal_manifest.contains("host-profile ="));
    assert!(std_manifest.contains("features = [\"host-profile\"]"));
    assert!(std_host.contains("signal_registry("));
}

#[test]
fn readiness_contract_names_the_platform_stop_line() {
    let readiness = include_str!("../../../docs/architecture/browser-readiness.md");
    for required in [
        "contract/source -> contract/sink",
        "Controlled composite transport",
        "Parent-facing events",
        "cargo check -p conduit-signal --no-default-features",
        "Browser, Pico W, WebSocket, TCP, UDP",
    ] {
        assert!(
            readiness.contains(required),
            "missing readiness item: {required}"
        );
    }
}
