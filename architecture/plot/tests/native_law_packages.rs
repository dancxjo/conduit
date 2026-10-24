//! Exercise the real checked-package export/install boundary, not a manual Back.
use conduit_plot::*;
fn package(source: &str, export: &str) -> StartupCatalog {
    let manifest = format!("pack custody/example (\n version = 1.0.0\n) {{\n ship {export}\n}}\n");
    let parsed = parse_syntax_document(&manifest);
    let sources = [PackageMemberSource {
        path: "main",
        source,
    }];
    let bundle =
        CheckedPackageBundle::from_sources(&manifest, &parsed.packages[0], &sources).unwrap();
    let exports =
        PackageExportCatalog::from_bundle(&bundle, &manifest, &parsed.packages[0], &sources)
            .unwrap();
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    catalog
}
#[test]
fn actual_shipped_export_and_native_alias_retain_source_law_custody() {
    let catalog = package(
        "type Counted = {\n value: U64\n where .value <= 17\n}\n",
        "Counted",
    );
    let witness = catalog
        .native_law_witness("custody/example/Counted")
        .unwrap();
    let original = witness.material().unwrap();
    assert!(NativeLawWitness::verify(&original, &witness.checked_type().value_type).is_ok());
    let mut omitted = original.clone();
    omitted.claimed.invariants.clear();
    assert!(NativeLawWitness::verify(&omitted, &witness.checked_type().value_type).is_err());
    let mut substituted = original;
    substituted.source = substituted.source.replace("<= 17", "<= 18");
    assert!(NativeLawWitness::verify(&substituted, &witness.checked_type().value_type).is_err());
    let doc = check_syntax_document(
        &parse_syntax_document(
            "with custody/example/Counted as Aliased\ntype Holder = {\n value: Aliased\n}\n",
        ),
        &catalog,
    )
    .unwrap();
    let holder = doc.native_law_witness("Holder").unwrap();
    assert!(NativeLawWitness::verify(
        &holder.material().unwrap(),
        &holder.checked_type().value_type
    )
    .is_ok());
}
#[test]
fn actual_imported_generic_parameter_annotation_keeps_its_owner_laws() {
    let catalog = package("type Bound = U16 in 1..=64\ntype Matrix<N: Bound> = {\n size: U16 in N..=N\n where .size == N\n}\n", "Matrix");
    let source = "with custody/example/Matrix as Grid\ntype Value = Grid<32>\n";
    let imported = check_syntax_document(&parse_syntax_document(source), &catalog).unwrap();
    let local = check_syntax_document(&parse_syntax_document("type Bound = U16 in 1..=64\ntype Matrix<N: Bound> = {\n size: U16 in N..=N\n where .size == N\n}\ntype Value = Matrix<32>\n"), &StartupCatalog::new()).unwrap();
    assert_eq!(
        imported.native_types,
        local
            .native_types
            .into_iter()
            .filter(|ty| ty.name == "Value")
            .collect::<Vec<_>>()
    );
    assert!(check_syntax_document(
        &parse_syntax_document(&source.replace("Grid<32>", "Grid<0>")),
        &catalog
    )
    .is_err());
    let witness = imported
        .native_law_witness("Value")
        .expect("original package family custody");
    let anchors = witness.family_anchors().unwrap();
    let material = witness.material().unwrap();
    assert!(matches!(
        NativeLawWitness::verify(&material, &witness.checked_type().value_type),
        Err(NativeLawCustodyRefusal::MissingContext)
    ));
    assert!(NativeLawWitness::verify_with_context(
        &material,
        &witness.checked_type().value_type,
        &NativeLawContext { families: &anchors }
    )
    .is_ok());
}
#[test]
fn actual_preparation_catalog_and_witness_preserve_send_sync_on_atomic_hosts() {
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<StartupCatalog>();
    require_send_sync::<NativeLawWitness>();
}
