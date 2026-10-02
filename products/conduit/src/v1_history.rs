//! Locked identity proof for Conduitese v1 sources accepted before #4375.

use serde::Deserialize;

const MANIFEST: &str = include_str!("../tests/fixtures/v1-history/manifest.json");

#[derive(Deserialize)]
struct HistoryManifest {
    schema: String,
    source_commit: String,
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

fn source(fixture: &str) -> &'static str {
    match fixture {
        "hello.conduit" => include_str!("../tests/fixtures/v1-history/hello.conduit"),
        "greet.conduit" => include_str!("../tests/fixtures/v1-history/greet.conduit"),
        "count.conduit" => include_str!("../tests/fixtures/v1-history/count.conduit"),
        other => panic!("unreviewed v1 history fixture {other}"),
    }
}

#[test]
fn accepted_v1_sources_retain_exact_source_checked_and_expanded_truth() {
    let manifest: HistoryManifest = serde_json::from_str(MANIFEST).unwrap();
    assert_eq!(manifest.schema, "conduit.plot/v1-history-corpus@1");
    assert_eq!(
        manifest.source_commit,
        "92b9561897f0b82e7dd0f84041b0b3581e9c75f5"
    );
    assert_eq!(manifest.programs.len(), 3);

    for expected in manifest.programs {
        let authored = source(&expected.fixture);
        let loaded = crate::plot_source::parse(authored).unwrap();
        assert_eq!(loaded.source, authored);
        assert_eq!(loaded.syntax.round_trip(), authored);

        let expanded = loaded.expand_entry().unwrap();
        assert_eq!(expanded.name, expected.entry);
        assert_eq!(
            expanded.source_document_id.as_str(),
            expected.source_document_id
        );
        assert_eq!(expanded.checked_plot_id.as_str(), expected.checked_plot_id);
        assert_eq!(
            expanded.expanded_plot_id.as_str(),
            expected.expanded_plot_id
        );
    }
}
