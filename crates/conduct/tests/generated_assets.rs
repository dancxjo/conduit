use std::path::{Path, PathBuf};
use std::process::Command;

const OUTPUT_FIXTURE: &str = include_str!("../../../conformance/c3/conduct-output-v1.json");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: impl AsRef<Path>) -> String {
    std::fs::read_to_string(root().join(relative)).unwrap()
}

#[test]
fn checked_in_assets_match_the_shared_command_model() {
    let output = Command::new(env!("CARGO_BIN_EXE_generate-conduct-assets"))
        .arg("--check")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let fixture: serde_json::Value = serde_json::from_str(OUTPUT_FIXTURE).unwrap();
    for case in fixture["generated_asset_cases"].as_array().unwrap() {
        let path = case["path"].as_str().unwrap();
        let generated = read(path);
        assert!(!generated.is_empty(), "{path}");
        let searchable = generated.replace("\\-", "-");
        for option_name in [
            "check",
            "explain",
            "run",
            "format",
            "diagnostic-format",
            "color",
            "quiet",
            "verbose-diagnostics",
        ] {
            assert!(searchable.contains(option_name), "{path}: {option_name}");
        }
    }

    let manual = read("generated/man/conduct.1");
    for section in ["STREAMS", "MACHINE OUTPUT", "EXIT STATUS"] {
        assert!(manual.contains(section), "{section}");
    }
}
