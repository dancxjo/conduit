use std::{fs, path::Path};

const ROOTS: &[&str] = &[
    "apps",
    "crates",
    "firmware",
    "hosts",
    "mechanisms",
    "targets",
];
const MARKERS: &[&str] = &[
    "capstone", "takeover", "r1_", "s4_", "a0_", "a1_", "a2_", "a3_", "a4_",
];

// These are exact, reviewed boundaries rather than generic permission for a
// directory to accumulate more milestone vocabulary.
const ALLOWLIST: &[(&str, &str, &str)] = &[
    (
        "apps/patchbay/model/src/presenter_plans.rs",
        "capstone",
        "accepted canonical Form identity is preserved without migration",
    ),
    (
        "apps/pete/src/interaction_convergence.rs",
        "capstone",
        "accepted Body Plan and Play identities are preserved without migration",
    ),
    (
        "crates/conduit-audio/src/sound_info.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
    (
        "crates/conduit-midi/src/adapter.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
    (
        "crates/conduit-semantic-catalog/src/music_input.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
    (
        "crates/conduit-system-continuity/src/r1_control_planning.rs",
        "r1_",
        "feature-gated R1 recovery conformance retained by issue 1798",
    ),
    (
        "crates/conduit-system-continuity/src/r1_planning.rs",
        "r1_",
        "feature-gated R1 recovery conformance retained by issue 1798",
    ),
    (
        "crates/conduit-system-continuity/src/r1_recovery.rs",
        "r1_",
        "feature-gated R1 recovery conformance retained by issue 1798",
    ),
    (
        "crates/conduit-system-continuity/src/lib.rs",
        "r1_",
        "feature-gated R1 recovery conformance exports retained by issue 1798",
    ),
    (
        "firmware/conduit-pico-w-signal/build.rs",
        "r1_",
        "exact proof-image generation retained by issue 1798",
    ),
    (
        "firmware/conduit-pico-w-signal/build.rs",
        "capstone",
        "exact physical proof-image generation retains its artifact identity",
    ),
    (
        "firmware/conduit-pico-w-signal/src/websocket_route.rs",
        "r1_",
        "R1 conformance frame bound retained by issue 1798",
    ),
    (
        "firmware/conduit-pico-w-signal/src/websocket_transport.rs",
        "r1_",
        "R1 conformance frame bound retained by issue 1798",
    ),
    (
        "hosts/std/src/r1_control.rs",
        "r1_",
        "exact R1 control participant retained by issue 1798",
    ),
    (
        "hosts/std/src/lib.rs",
        "r1_",
        "exact R1 control exports retained by issue 1798",
    ),
    (
        "hosts/std/src/pico_control_source.rs",
        "r1_",
        "exact R1 control participant retained by issue 1798",
    ),
    (
        "hosts/std/src/r1_control_input.rs",
        "r1_",
        "exact R1 control participant retained by issue 1798",
    ),
    (
        "hosts/std/src/hosted_midi/mod.rs",
        "a4_",
        "A4 is the musical tuning reference, not a proof rung",
    ),
];

#[test]
fn durable_source_does_not_gain_proof_history_api_names() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has repository parent");
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
