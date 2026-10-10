use crate::prelude::*;
use crate::{
    parse_syntax_document, CheckedPackageBundle, PackageExportCatalog, PackageMemberSource,
    StartupCatalog,
};

fn exports(manifest: &str, members: &[PackageMemberSource<'_>]) -> PackageExportCatalog {
    let syntax = parse_syntax_document(manifest);
    let bundle =
        CheckedPackageBundle::from_sources(manifest, &syntax.packages[0], members).unwrap();
    PackageExportCatalog::from_bundle(&bundle, manifest, &syntax.packages[0], members).unwrap()
}

#[test]
fn original_owner_modules_survive_aliases_and_package_reexports() {
    let domain = "# π / t͡ʃ, original source bytes\ntype Dimension = U16 in 1..=64\n";
    let vector = "type Vector<N: Dimension> = collection U8 = N\n";
    let members = [
        PackageMemberSource {
            path: "types/domain",
            source: domain,
        },
        PackageMemberSource {
            path: "types/vector",
            source: vector,
        },
    ];
    let upstream = exports(
        "pack example/vector (\n version = 1.0.0\n) {\n ship Vector\n}\n",
        &members,
    );
    let mut catalog = StartupCatalog::new();
    upstream.install_shipped_types(&mut catalog).unwrap();
    let origin = catalog
        .native_family_sources("example/vector/Vector")
        .unwrap()
        .find(|origin| origin.declaration_name == "Dimension")
        .unwrap();
    assert_eq!(origin.module_path, "types/domain");
    assert_eq!(
        origin.source_document_id,
        parse_syntax_document(domain).source_document_id()
    );
    assert_eq!(
        &domain[origin.declaration_span.start..origin.declaration_span.end],
        "type Dimension = U16 in 1..=64"
    );
    assert_eq!(
        origin.package_content_digest,
        upstream.package_content_digest()
    );
    let history = "with example/vector/Vector as Cells\ntype History<H: U16, D: U16> = collection Cells<D> = H\n";
    let downstream = exports(
        "pack example/history (\n version = 2.0.0\n) {\n ship History\n}\n",
        &[PackageMemberSource {
            path: "history/main",
            source: history,
        }],
    );
    downstream.install_shipped_types(&mut catalog).unwrap();
    let origins = catalog
        .native_family_sources("example/history/History")
        .unwrap()
        .collect::<Vec<_>>();
    for name in ["Dimension", "Vector"] {
        let origin = origins
            .iter()
            .find(|origin| origin.declaration_name == name)
            .unwrap();
        assert_eq!(
            origin.package_content_digest,
            upstream.package_content_digest()
        );
    }
    let root = origins
        .iter()
        .find(|origin| origin.declaration_name == "History")
        .unwrap();
    assert_eq!(root.module_path, "history/main");
    assert_eq!(
        root.source_document_id,
        parse_syntax_document(history).source_document_id()
    );
    assert_eq!(
        root.package_content_digest,
        downstream.package_content_digest()
    );
    let consumer = parse_syntax_document(
        "with example/history/History as Archive\ntype Value = Archive<2, 32>\n",
    );
    let aliased = crate::native_type::install_import_aliases(&consumer, &catalog).unwrap();
    assert_eq!(
        aliased
            .native_family_sources("Archive")
            .unwrap()
            .collect::<Vec<_>>(),
        origins
    );
}

#[test]
fn changed_module_bytes_cannot_supply_provenance_for_a_checked_bundle() {
    let manifest = "pack example/vector (\n version = 1.0.0\n) {\n ship Vector\n}\n";
    let syntax = parse_syntax_document(manifest);
    let original = [PackageMemberSource {
        path: "main",
        source: "type Vector<N: U16> = collection U8 = N\n",
    }];
    let bundle =
        CheckedPackageBundle::from_sources(manifest, &syntax.packages[0], &original).unwrap();
    let changed = [PackageMemberSource {
        path: "main",
        source: "# changed\ntype Vector<N: U16> = collection U8 = N\n",
    }];
    assert!(
        PackageExportCatalog::from_bundle(&bundle, manifest, &syntax.packages[0], &changed)
            .is_err()
    );
}
