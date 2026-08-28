use std::path::Path;

#[test]
fn core_has_no_upward_domain_or_application_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "conduit-alife",
        "conduit-audio",
        "conduit-human",
        "conduit-robotics",
        "conduit-time",
        "conduit-web",
        "patchbay-control",
        "semantics/",
        "apps/",
        "targets/",
        "proof/",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "conduit-core must not depend upward on {forbidden}"
        );
    }
}

#[test]
fn extracted_domain_modules_cannot_reappear_in_core() {
    let source = include_str!("../src/lib.rs");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for module in [
        "human_interaction",
        "human_media",
        "input_chord",
        "input_keymap",
        "json_info",
        "key_event",
        "patchbay_control",
        "robotics_hazard_info",
        "robotics_info",
        "robotics_input_info",
    ] {
        assert!(
            !source.contains(&format!("mod {module};")),
            "extracted domain module {module} returned to the core facade"
        );
        assert!(
            !root.join(format!("{module}.rs")).exists() && !root.join(module).exists(),
            "extracted domain source {module} returned to architecture/core/src"
        );
    }
}

#[test]
fn checked_responsibility_map_names_every_retained_module() {
    let source = include_str!("../src/lib.rs");
    let map = include_str!("../README.md");
    for line in source.lines() {
        let Some(module) = line
            .strip_prefix("mod ")
            .and_then(|line| line.strip_suffix(';'))
        else {
            continue;
        };
        if module == "tests" {
            continue;
        }
        assert!(
            map.contains(&format!("`{module}.rs`"))
                || map.contains(&format!("`{module}.rs` and children")),
            "retained core module {module} is absent from the responsibility map"
        );
    }
}
