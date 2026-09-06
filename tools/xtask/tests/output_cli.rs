use serde_json::Value;
use std::process::{Command, Output};

fn xtask(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(arguments)
        .output()
        .expect("xtask must run")
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be one JSON object: {error}; stdout={:?}; stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn dry_run_json_is_deterministic_and_effect_free_for_migrated_commands() {
    let commands: &[&[&str]] = &[
        &["--dry-run", "--json", "midi", "list"],
        &["--dry-run", "--json", "audio", "list"],
        &[
            "--dry-run",
            "--json",
            "audio",
            "playback-proof",
            "--card-id",
            "Fixture",
            "--device",
            "7",
            "--authorize-output",
        ],
        &["--dry-run", "--json", "pico", "build"],
        &[
            "--dry-run",
            "--json",
            "pico",
            "drive-create",
            "--wheels-off-floor",
        ],
    ];
    for command in commands {
        let first = xtask(command);
        let second = xtask(command);
        assert!(
            first.status.success(),
            "{command:?}: {:?}",
            String::from_utf8_lossy(&first.stderr)
        );
        assert_eq!(first.stdout, second.stdout, "{command:?}");
        let report = parse_stdout(&first);
        assert_eq!(report["effects_performed"], false, "{command:?}");
        assert!(report["schema"]
            .as_str()
            .is_some_and(|schema| schema.ends_with("@1")));
    }
}

#[test]
fn quiet_dry_runs_emit_no_ordinary_stdout() {
    for command in [
        vec!["--dry-run", "--quiet", "midi", "list"],
        vec!["--dry-run", "--quiet", "audio", "list"],
        vec!["--dry-run", "--quiet", "pico", "build"],
    ] {
        let output = xtask(&command);
        assert!(
            output.status.success(),
            "{command:?}: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty(), "{command:?}");
    }
}

#[test]
fn demo_tour_dry_run_uses_the_canonical_tour_product_route() {
    let output = xtask(&["--dry-run", "demo", "tour"]);
    assert!(
        output.status.success(),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("target/tour-product"), "{stdout}");
    assert!(stdout.contains("--mount /tour/"), "{stdout}");
    assert!(!stdout.contains("target/book-product"), "{stdout}");
    assert!(!stdout.contains("--mount /book/"), "{stdout}");
}

#[test]
fn live_pico_structured_modes_refuse_before_dispatch() {
    let json = xtask(&["--json", "pico", "drive-create", "--wheels-off-floor"]);
    assert!(!json.status.success());
    let report = parse_stdout(&json);
    assert_eq!(report["schema"], "conduit.tools/xtask/output-refusal@1");
    assert_eq!(report["disposition"], "unsupported-before-dispatch");
    assert_eq!(report["capability"], "json");

    let quiet = xtask(&["--quiet", "pico", "drive-create", "--wheels-off-floor"]);
    assert!(!quiet.status.success());
    assert!(quiet.stdout.is_empty());
}

#[test]
fn migrated_sources_have_no_bespoke_unsupported_literals() {
    for source in [
        include_str!("../src/main.rs"),
        include_str!("../src/commands/midi.rs"),
        include_str!("../src/commands/audio.rs"),
        include_str!("../../../targets/rp2040/firmware/pico-w-signal/fabrication/xtask/mod.rs"),
    ] {
        assert!(!source.contains("--json is not yet supported"));
        assert!(!source.contains("--quiet is not yet supported"));
    }
}
