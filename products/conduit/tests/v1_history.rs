use serde::Deserialize;
use std::process::Command;

const MANIFEST: &str = include_str!("fixtures/v1-history/manifest.json");

#[derive(Deserialize)]
struct HistoryManifest {
    programs: Vec<HistoryProgram>,
}

#[derive(Deserialize)]
struct HistoryProgram {
    fixture: String,
}

#[test]
fn conduit_inspect_refuses_archived_executable_form_syntax() {
    let manifest: HistoryManifest = serde_json::from_str(MANIFEST).unwrap();
    let fixture_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1-history");

    for expected in manifest.programs {
        let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(fixture_root.join(&expected.fixture))
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "archived syntax must not become a compatibility alias"
        );
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("CND-FRM-019"), "{diagnostic}");
    }
}
