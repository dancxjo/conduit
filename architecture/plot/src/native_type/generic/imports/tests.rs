use crate::{
    check_syntax_document, parse_syntax_document, CheckedPackageBundle, PackageExportCatalog,
    PackageMemberSource, StartupCatalog,
};

fn catalog(source: &str, name: &str) -> StartupCatalog {
    let manifest_source =
        alloc::format!("pack example/families (\n version = 1.0.0\n) {{\n ship {name}\n}}\n");
    let parsed = parse_syntax_document(&manifest_source);
    let sources = [PackageMemberSource {
        path: "main",
        source,
    }];
    let bundle =
        CheckedPackageBundle::from_sources(&manifest_source, &parsed.packages[0], &sources)
            .unwrap();
    let exports =
        PackageExportCatalog::from_bundle(&bundle, &manifest_source, &parsed.packages[0], &sources)
            .unwrap();
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    catalog
}
fn value(source: &str, catalog: &StartupCatalog) -> crate::CheckedNativeType {
    check_syntax_document(&parse_syntax_document(source), catalog)
        .unwrap()
        .native_types
        .into_iter()
        .find(|value| value.name == "Value")
        .unwrap()
}

#[test]
fn mixed_imported_family_preserves_variable_bounds_and_nominal_declarations() {
    let owner = "type Tier<T, N: U16> = sequence T in 0..=N\n";
    let catalog = catalog(owner, "Tier");
    let source = "with example/families/Tier as Events\ntype Value = Events<U16, 64>\n";
    let imported = value(source, &catalog);
    let local = value(
        &alloc::format!("{owner}type Value = Tier<U16, 64>\n"),
        &StartupCatalog::new(),
    );
    assert_eq!(imported, local);
    let prepared = crate::prepare_source_types(&parse_syntax_document(source), &catalog).unwrap();
    assert_eq!(prepared.named_type("Value"), Some(&imported.value_type));
    let other = check_syntax_document(
        &parse_syntax_document(&source.replace("type Value", "type Other")),
        &catalog,
    )
    .unwrap()
    .native_types
    .into_iter()
    .find(|value| value.name == "Other")
    .unwrap();
    assert_ne!(imported.identity, other.identity);
}

#[test]
fn imported_family_keeps_parameter_and_instantiated_record_laws() {
    let owner = "type Dimension = U16 where . > 0\ntype Matrix<N: Dimension> = {\n size: U16 in N..=N\n where .size == N\n}\n";
    let catalog = catalog(owner, "Matrix");
    let source = "with example/families/Matrix as Block\ntype Value = Block<32>\n";
    let imported = value(source, &catalog);
    let local = value(
        &alloc::format!("{owner}type Value = Matrix<32>\n"),
        &StartupCatalog::new(),
    );
    assert_eq!(imported, local);
    assert_eq!(imported.invariants.len(), 1);
    let failure = check_syntax_document(
        &parse_syntax_document(&source.replace("Block<32>", "Block<0>")),
        &catalog,
    )
    .unwrap_err();
    assert!(failure.message.contains("Type law"), "{}", failure.message);
}

#[test]
fn owner_source_trivia_is_not_family_meaning_but_changed_contract_is() {
    let owner = "type Dimension = U16 in 1..=64\ntype Vector<N: Dimension> = collection U8 = N\n";
    let source = "with example/families/Vector as Samples\ntype Value = Samples<32>\n";
    let first = value(source, &catalog(owner, "Vector"));
    let spaced = value(
        source,
        &catalog(&owner.replace("collection U8", "collection  U8"), "Vector"),
    );
    assert_eq!(first, spaced);
    let changed = value(
        source,
        &catalog(&owner.replace("1..=64", "1..=128"), "Vector"),
    );
    assert_ne!(first.identity, changed.identity);
}

#[test]
fn import_alias_collisions_unused_aliases_and_unknown_families_refuse() {
    let catalog = catalog("type Vector<N: U16> = collection U8 = N\n", "Vector");
    for source in [
        "with example/families/Vector as Samples\ntype Value = U16\n",
        "with example/families/Vector as Samples\ntype Samples = U16\ntype Value = Samples<2>\n",
        "with example/families/Missing as Samples\ntype Value = Samples<2>\n",
        "with example/families/Vector as Samples\nwith example/families/Vector as Samples\ntype Value = Samples<2>\n",
    ] {
        assert!(check_syntax_document(&parse_syntax_document(source), &catalog).is_err(), "{source}");
    }
}
