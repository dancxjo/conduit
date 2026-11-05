//! Canonical compatibility and bounded identity/snapshot preparation costs.
use conduit_plot::*;
use sha2::{Digest, Sha256};
#[path = "common/allocation_probe.rs"]
mod allocation;
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;
fn legacy_identity(source: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("canonical-source:{source}").as_bytes())
    )
}
#[test]
fn streaming_identity_preserves_the_existing_parser_byte_domain() {
    assert_eq!(
        canonical_source_document_id("plot main { }\n").as_str(),
        "5deda436446476d59c3046840e379ab0cf1c3e58a86eac1ebf979bd6e37cb5c7"
    );
    for source in [
        "",
        "plot main { }",
        "# α 🦀\r\nplot main { }\r\n",
        "\u{feff}plot main { }",
        "unfinished {\0",
    ] {
        let actual = canonical_source_document_id(source);
        assert_eq!(actual.as_str(), legacy_identity(source));
        assert_eq!(actual, parse_syntax_document(source).source_document_id());
    }
    assert_ne!(
        canonical_source_document_id("plot main { }"),
        canonical_source_document_id("plot main { }\n")
    );
}
#[test]
fn identity_cost_is_one_fixed_result_allocation_without_a_source_copy() {
    for size in [0, 1024, MAXIMUM_PLOT_SOURCE_BYTES] {
        let source = "x".repeat(size);
        let expected = legacy_identity(&source);
        let (actual, observed) = allocation::observe(|| canonical_source_document_id(&source));
        assert_eq!(actual.as_str(), expected);
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.peak_bytes, 64);
        assert_eq!(observed.requested_bytes, 64);
    }
}
#[cfg(target_has_atomic = "ptr")]
#[test]
fn immutable_snapshot_does_not_parse_source_before_the_admitted_check() {
    for size in [1024, MAXIMUM_PLOT_SOURCE_BYTES] {
        // Many lexical boundaries would demand a large token/AST workspace.
        // Snapshot custody must not invoke any of that machinery.
        let source = "x ".repeat(size / 2);
        let context = SourceCheckContext::new(
            CheckContextRevision {
                grammar_edition: "conduitese-v1".into(),
                catalog: "test/empty@1".into(),
                kinds_fores: "test/empty@1".into(),
                types_laws: "test/empty@1".into(),
                backs: "test/empty@1".into(),
            },
            StartupCatalog::new(),
            ProfileCatalog::new(),
            CanonicalBackCatalog::new(),
        )
        .unwrap();
        let (projection, observed) = allocation::observe(|| {
            SourceCheckProjection::new("test/owned-source", 1, &source, context).unwrap()
        });
        assert_eq!(
            projection.snapshot().source_document_id,
            legacy_identity(&source)
        );
        assert_eq!(
            projection.snapshot().sha256,
            format!("{:x}", Sha256::digest(source.as_bytes()))
        );
        assert!(observed.peak_bytes <= source.len() + 4096, "{observed:?}");
        println!("snapshot {} bytes: {observed:?}", source.len());
        if size == MAXIMUM_PLOT_SOURCE_BYTES {
            let (old_id, old) = allocation::observe(|| {
                let document = parse_syntax_document(&source);
                legacy_identity(document.round_trip())
            });
            assert_eq!(old_id, projection.snapshot().source_document_id);
            println!("former parse-for-identity step: {old:?}");
        }
    }
}
