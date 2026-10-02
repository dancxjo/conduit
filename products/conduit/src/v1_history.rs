//! Archived source identity proof; executable `form` is no longer accepted.

use serde::Deserialize;
use sha2::{Digest, Sha256};

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
    source_document_id: String,
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
fn archived_v1_sources_retain_identity_and_are_explicitly_refused() {
    let manifest: HistoryManifest = serde_json::from_str(MANIFEST).unwrap();
    assert_eq!(manifest.schema, "conduit.form/v1-history-corpus@1");
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

        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(format!("canonical-source:{authored}"))
            ),
            expected.source_document_id
        );
        let diagnostic = loaded
            .check()
            .err()
            .expect("historical executable form is refused");
        assert!(diagnostic.contains("CND-FRM-019"), "{diagnostic}");
    }
}
