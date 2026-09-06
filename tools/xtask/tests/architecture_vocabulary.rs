use std::{fs, path::Path};

const ROOTS: &[&str] = &[
    "architecture",
    "bodies",
    "mechanisms",
    "products",
    "semantics",
    "targets",
];
const MARKERS: &[&str] = &[
    "capstone", "takeover", "r1_", "s4_", "a0_", "a1_", "a2_", "a3_", "a4_",
];

// These are exact, reviewed boundaries rather than generic permission for a
// directory to accumulate more milestone vocabulary.
const ALLOWLIST: &[(&str, &str, &str)] = &[
    (
        "products/patchbay/model/src/presenter_plans.rs",
        "capstone",
        "accepted canonical Form identity is preserved without migration",
    ),
    (
        "bodies/pete/src/interaction_convergence.rs",
        "capstone",
        "accepted Body Plan and Play identities are preserved without migration",
    ),
    (
        "products/patchbay/native/src/bin/browser_parts_capstone/physical_body.rs",
        "r1_",
        "physical proof composition consumes the explicitly proof-owned R1 contract",
    ),
    (
        "semantics/audio/src/sound_info.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
    (
        "mechanisms/protocols/midi/src/adapter.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
    (
        "semantics/catalog/src/music_input.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
    (
        "targets/rp2040/firmware/pico-w-signal/build.rs",
        "r1_",
        "exact proof-image generation retained by issue 1798",
    ),
    (
        "targets/rp2040/firmware/pico-w-signal/build.rs",
        "capstone",
        "exact physical proof-image generation retains its artifact identity",
    ),
    (
        "targets/rp2040/firmware/pico-w-signal/src/websocket_route.rs",
        "r1_",
        "R1 conformance frame bound retained by issue 1798",
    ),
    (
        "targets/rp2040/firmware/pico-w-signal/src/websocket_transport.rs",
        "r1_",
        "R1 conformance frame bound retained by issue 1798",
    ),
    (
        "targets/std/src/r1_control.rs",
        "r1_",
        "exact R1 control participant retained by issue 1798",
    ),
    (
        "targets/std/src/lib.rs",
        "r1_",
        "exact R1 control exports retained by issue 1798",
    ),
    (
        "targets/std/src/pico_control_source.rs",
        "r1_",
        "exact R1 control participant retained by issue 1798",
    ),
    (
        "targets/std/src/r1_control_input.rs",
        "r1_",
        "exact R1 control participant retained by issue 1798",
    ),
    (
        "targets/std/src/hosted_midi/mod.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
];

#[test]
fn durable_source_does_not_gain_proof_history_api_names() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");
    let mut violations = Vec::new();
    for root in ROOTS {
        inspect_directory(repository, &repository.join(root), &mut violations);
    }
    assert!(
        violations.is_empty(),
        "proof-history vocabulary escaped its explicit boundary:\n{}",
        violations.join("\n")
    );
}

fn inspect_directory(repository: &Path, directory: &Path, violations: &mut Vec<String>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("read {} entry: {error}", directory.display()));
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if is_explicit_boundary(repository, &path) {
                continue;
            }
            inspect_directory(repository, &path, violations);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            inspect_source(repository, &path, violations);
        }
    }
}

fn is_explicit_boundary(repository: &Path, path: &Path) -> bool {
    let relative = path
        .strip_prefix(repository)
        .expect("path beneath repository");
    relative.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("proof" | "proof-appliances" | "tests" | "fixtures" | "fabrication" | "build")
        )
    })
}

fn inspect_source(repository: &Path, path: &Path, violations: &mut Vec<String>) {
    let relative = path
        .strip_prefix(repository)
        .expect("source beneath repository")
        .to_string_lossy()
        .replace('\\', "/");
    if relative.contains("/src/bin/pete_capstone/")
        || relative.ends_with("/src/bin/pete_capstone.rs")
    {
        return;
    }
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    for (line_index, line) in source.lines().enumerate() {
        if !is_api_declaration(line) {
            continue;
        }
        let lowercase = line.to_ascii_lowercase();
        for marker in MARKERS {
            if lowercase.contains(marker) && !is_allowed(&relative, marker) {
                violations.push(format!("{}:{} contains {marker}", relative, line_index + 1));
            }
        }
    }
}

fn is_api_declaration(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return false;
    }
    [
        "pub ", "pub(", "mod ", "const ", "static ", "fn ", "struct ", "enum ", "type ", "use ",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

fn is_allowed(path: &str, marker: &str) -> bool {
    ALLOWLIST
        .iter()
        .any(|(allowed_path, allowed_marker, reason)| {
            assert!(!reason.is_empty(), "allowlist entries require a reason");
            path == *allowed_path && marker == *allowed_marker
        })
}
