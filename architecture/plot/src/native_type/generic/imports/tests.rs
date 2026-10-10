use crate::{
    check_syntax_document, parse_syntax_document, CheckedPackageBundle, PackageExportCatalog,
    PackageMemberSource, StartupCatalog,
};

fn catalog(source: &str, name: &str) -> StartupCatalog {
    catalog_with_exports(source, &[name])
}

fn catalog_with_exports(source: &str, names: &[&str]) -> StartupCatalog {
    catalog_with_base(source, names, StartupCatalog::new())
}

fn catalog_with_base(source: &str, names: &[&str], mut catalog: StartupCatalog) -> StartupCatalog {
    let ships = names
        .iter()
        .map(|name| alloc::format!(" ship {name}\n"))
        .collect::<alloc::string::String>();
    let manifest_source =
        alloc::format!("pack example/families (\n version = 1.0.0\n) {{\n{ships}}}\n");
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

#[test]
fn checked_type_argument_import_alias_does_not_change_value_family_identity() {
    let owner = "type Sample = U16 in 1..=64\ntype Tier<T, N: U16> = sequence T in 0..=N\n";
    let catalog = catalog_with_exports(owner, &["Sample", "Tier"]);
    let source = "with example/families/Sample as Reading\nwith example/families/Tier as Events\ntype Value = Events<Reading, 4>\n";
    let imported = value(source, &catalog);
    let renamed = value(&source.replace("Reading", "Measured"), &catalog);
    let local = value(
        &alloc::format!("{owner}type Value = Tier<Sample, 4>\n"),
        &StartupCatalog::new(),
    );
    assert_eq!(imported, renamed);
    assert_eq!(imported, local);
}

#[test]
fn nested_mixed_type_arguments_have_the_same_owner_and_imported_meaning() {
    let owner =
        "type Cell<T, N: U16> = collection T = N\ntype Grid<T, N: U16> = collection T = N\n";
    let catalog = catalog_with_exports(owner, &["Cell", "Grid"]);
    let source = "with example/families/Cell as Inner\nwith example/families/Grid as Outer\ntype Value = Outer<Inner<U16, 2>, 4>\n";
    let imported = value(source, &catalog);
    let renamed = value(
        &source.replace("Inner", "Rows").replace("Outer", "Matrix"),
        &catalog,
    );
    let local = value(
        &alloc::format!("{owner}type Value = Grid<Cell<U16, 2>, 4>\n"),
        &StartupCatalog::new(),
    );
    assert_eq!(imported, renamed);
    assert_eq!(imported, local);
}

#[test]
fn local_closed_type_arguments_and_inline_refinements_are_checked() {
    let owner = "type Grid<T, N: U16> = collection T = N\n";
    let catalog = catalog(owner, "Grid");
    let source = "with example/families/Grid as Samples\ntype Reading = U16 in 1..=64\ntype Value = Samples<Reading, 4>\n";
    let imported = value(source, &catalog);
    let local = value(
        &alloc::format!("{owner}type Reading = U16 in 1..=64\ntype Value = Grid<Reading, 4>\n"),
        &StartupCatalog::new(),
    );
    assert_eq!(imported, local);
    let inline = value(
        "with example/families/Grid as Samples\ntype Value = Samples<U16 in 1..=64, 4>\n",
        &catalog,
    );
    let changed = value(
        "with example/families/Grid as Samples\ntype Value = Samples<U16 in 1..=128, 4>\n",
        &catalog,
    );
    assert_ne!(inline.identity, changed.identity);
    let recursive = "with example/families/Grid as Samples\ntype Value = Samples<Value, 4>\n";
    assert!(check_syntax_document(&parse_syntax_document(recursive), &catalog).is_err());
}

#[test]
fn downstream_package_family_retains_explicit_upstream_family_imports() {
    let upstream = "type Dimension = U16 in 1..=64\ntype Sample = U8 in 1..=255\ntype Vector<N: Dimension> = collection Sample = N\n";
    let upstream_catalog = catalog(upstream, "Vector");
    let source = "with example/families/Vector as Cells\ntype History<H: U16, D: U16> = collection Cells<D> = H\n";
    let manifest_source = "pack example/history (\n version = 1.0.0\n) {\n ship History\n}\n";
    let manifest = parse_syntax_document(manifest_source);
    let sources = [PackageMemberSource {
        path: "main",
        source,
    }];
    let bundle =
        CheckedPackageBundle::from_sources(manifest_source, &manifest.packages[0], &sources)
            .unwrap();
    let exports = PackageExportCatalog::from_bundle(
        &bundle,
        manifest_source,
        &manifest.packages[0],
        &sources,
    )
    .unwrap();
    let mut downstream_catalog = upstream_catalog.clone();
    exports
        .install_shipped_types(&mut downstream_catalog)
        .unwrap();
    let consumer = "with example/history/History as Archive\ntype Value = Archive<2, 32>\n";
    let imported = value(consumer, &downstream_catalog);
    let owner = value(
        &alloc::format!("{source}type Value = History<2, 32>\n"),
        &upstream_catalog,
    );
    assert_eq!(imported, owner);
    let renamed_source = source.replace("Cells", "Rows");
    let renamed_sources = [PackageMemberSource {
        path: "main",
        source: &renamed_source,
    }];
    let renamed_bundle = CheckedPackageBundle::from_sources(
        manifest_source,
        &manifest.packages[0],
        &renamed_sources,
    )
    .unwrap();
    let renamed_exports = PackageExportCatalog::from_bundle(
        &renamed_bundle,
        manifest_source,
        &manifest.packages[0],
        &renamed_sources,
    )
    .unwrap();
    let mut renamed_catalog = upstream_catalog.clone();
    renamed_exports
        .install_shipped_types(&mut renamed_catalog)
        .unwrap();
    assert_eq!(value(consumer, &renamed_catalog), owner);
    let capsule = downstream_catalog.native_families["example/history/History"].clone();
    let mut isolated = StartupCatalog::new();
    isolated
        .native_families
        .insert("example/history/History".into(), capsule.clone());
    assert_eq!(value(consumer, &isolated), owner);
    let mut foreign = catalog(
        &upstream.replace("U8 in 1..=255", "U32 in 1..=255"),
        "Vector",
    );
    foreign
        .native_families
        .insert("example/history/History".into(), capsule);
    let combined = "with example/families/Vector as Foreign\nwith example/history/History as Archive\ntype Other = {\n samples: Foreign<32>\n}\ntype Value = Archive<2, 32>\n";
    assert_eq!(value(combined, &foreign), owner);
    let failure = check_syntax_document(
        &parse_syntax_document(&consumer.replace("Archive<2, 32>", "Archive<2, 65>")),
        &downstream_catalog,
    )
    .unwrap_err();
    assert!(
        failure.message.contains("scalar contract"),
        "{}",
        failure.message
    );
    crate::check_package_bundle(
        &bundle,
        manifest_source,
        &manifest.packages[0],
        &sources,
        &upstream_catalog,
    )
    .unwrap();
}

#[test]
fn dependency_identity_work_refuses_before_expanding_an_excessive_family() {
    let mut source = alloc::string::String::new();
    for child in 0..63 {
        source.push_str(&alloc::format!("type Child{child}<N: U16> = {{\n"));
        for field in 0..63 {
            source.push_str(&alloc::format!(" field{field}: U8\n"));
        }
        source.push_str(" marker: collection U8 = N\n}\n");
    }
    source.push_str("type Huge<N: U16> = {\n");
    for child in 0..63 {
        source.push_str(&alloc::format!(" child{child}: Child{child}<N>\n"));
    }
    source.push_str(" marker: collection U8 = N\n}\ntype Value = Huge<1>\n");
    let parsed = parse_syntax_document(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let failure = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
    assert!(
        failure.message.contains("finite work budget"),
        "{}",
        failure.message
    );
    assert!(!source[failure.span.start..failure.span.end].is_empty());
}

#[test]
fn family_capture_excludes_unrelated_local_and_ambient_declarations() {
    let ambient = catalog("type Ambient<N: U16> = collection U32 = N\n", "Ambient");
    let owner = "type Positive = U16 where . > 0\ntype Dimension = Positive\ntype Vector<N: Dimension> = collection U8 = N\ntype Unrelated<N: U16> = collection U64 = N\ntype UnrelatedValue = U32\n";
    let installed = catalog_with_base(owner, &["Vector"], ambient);
    let capsule = &installed.native_families["example/families/Vector"];
    assert_eq!(capsule.templates.len(), 1);
    assert_eq!(capsule.templates[0].name.text, "Vector");
    let names = capsule
        .dependencies
        .iter()
        .map(|value| value.name.as_str())
        .collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(
        names,
        alloc::collections::BTreeSet::from(["Dimension", "Positive"])
    );
    let mut isolated = StartupCatalog::new();
    isolated
        .native_families
        .insert("example/families/Vector".into(), capsule.clone());
    let source = "with example/families/Vector as Samples\ntype Value = Samples<2>\n";
    assert_eq!(
        value(source, &isolated),
        value(
            &alloc::format!("{owner}type Value = Vector<2>\n"),
            &StartupCatalog::new()
        )
    );
    let refusal = check_syntax_document(
        &parse_syntax_document(&source.replace("Samples<2>", "Samples<0>")),
        &isolated,
    )
    .unwrap_err();
    assert!(refusal.message.contains("Type law"), "{}", refusal.message);
}
