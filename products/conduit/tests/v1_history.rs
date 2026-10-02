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
    entry: String,
    source_document_id: String,
    checked_plot_id: String,
    expanded_plot_id: String,
}

#[test]
fn conduit_inspect_reports_locked_v1_identities() {
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
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let rendered = String::from_utf8(output.stdout).unwrap();
        let fields = rendered
            .lines()
            .filter_map(|line| line.split_once(' '))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(fields.get("Plot"), Some(&expected.entry.as_str()));
        assert_eq!(
            fields.get("source"),
            Some(&expected.source_document_id.as_str())
        );
        assert_eq!(
            fields.get("checked"),
            Some(&expected.checked_plot_id.as_str())
        );
        assert_eq!(
            fields.get("expanded"),
            Some(&expected.expanded_plot_id.as_str())
        );
    }
}
