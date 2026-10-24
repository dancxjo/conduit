//! Optional witness pressure must not become a new Source admission policy.
use super::*;
use crate::{check_syntax_document, parse_syntax_document, NativeLawWitness};

fn witness(source: &str, name: &str) -> NativeLawWitness {
    check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
        .unwrap()
        .native_law_witness(name)
        .unwrap()
        .clone()
}

fn full_catalog(witness: &NativeLawWitness) -> StartupCatalog {
    let mut catalog = StartupCatalog::new();
    // Populate the real installed Type and optional-custody registries together.
    // Direct test-only insertion avoids 4096 transactional whole-catalog clones;
    // every entry has the same valid representation as the public registration.
    for index in 0..4096 {
        let name = format!("owner/type-{index:04}");
        catalog
            .insert_checked_native_type(name.clone(), witness.checked_type())
            .unwrap();
        catalog.native_law_custody.insert(name, witness.clone());
    }
    assert_eq!(catalog.native_law_custody.len(), 4096);
    catalog
}

fn counted() -> NativeLawWitness {
    witness(
        "type Counted = {\n value: U64\n where .value <= 17\n}\n",
        "Counted",
    )
}

#[test]
fn full_optional_registry_preserves_real_source_import_type_and_law_admission() {
    let witness = counted();
    let catalog = full_catalog(&witness);
    let mut legacy = catalog.clone();
    legacy.native_law_custody.clear();
    let syntax = parse_syntax_document(
        "with owner/type-0000 as Aliased\ntype Holder = {\n value: Aliased\n}\n",
    );
    let expected = check_syntax_document(&syntax, &legacy).unwrap();
    let actual = check_syntax_document(&syntax, &catalog)
        .expect("optional witness capacity must not refuse a valid Source import");
    assert_eq!(actual.native_types, expected.native_types);
    assert_eq!(catalog.native_law_custody.len(), 4096);
    let aliases = crate::native_type::install_import_aliases(&syntax, &catalog).unwrap();
    assert!(aliases.native_law_witness("Aliased").is_none());
    assert_eq!(
        aliases.native_law_witness("owner/type-0000"),
        Some(&witness)
    );
    assert_eq!(
        aliases.structured_type_invariants("Aliased"),
        Some(witness.checked_type().invariants.as_slice())
    );
}

#[test]
fn full_optional_registry_preserves_actual_package_export_installation() {
    let original = counted();
    let mut catalog = full_catalog(&original);
    let mut legacy = catalog.clone();
    legacy.native_law_custody.clear();
    let manifest = "pack pressure/example (\n version = 1.0.0\n) {\n ship Exported\n}\n";
    let parsed = parse_syntax_document(manifest);
    let members = [crate::PackageMemberSource {
        path: "main",
        source: "type Exported = {\n value: U16\n where .value <= 9\n}\n",
    }];
    let bundle =
        crate::CheckedPackageBundle::from_sources(manifest, &parsed.packages[0], &members).unwrap();
    let exports =
        crate::PackageExportCatalog::from_bundle(&bundle, manifest, &parsed.packages[0], &members)
            .unwrap();
    let expected = exports.install_shipped_types(&mut legacy).unwrap();
    let actual = exports
        .install_shipped_types(&mut catalog)
        .expect("optional witness pressure must not refuse a checked package export");
    assert_eq!(actual, expected);
    assert!(catalog
        .native_law_witness("pressure/example/Exported")
        .is_none());
    assert_eq!(catalog.native_law_custody.len(), 4096);
    assert_eq!(
        catalog.structured_type_invariants("pressure/example/Exported"),
        Some(actual[0].invariants.as_slice())
    );
}

#[test]
fn pressure_keeps_mismatch_hard_and_explicit_registration_transactional() {
    let original = counted();
    let wrong = witness("type Wrong = U32\n", "Wrong");
    let mut catalog = full_catalog(&original);
    let before = catalog.clone();
    assert!(catalog
        .retain_native_law_witness("owner/type-0000", &wrong)
        .is_err());
    assert_eq!(catalog, before);
    assert!(catalog
        .insert_native_law_witness("overflow", &original)
        .is_err());
    assert_eq!(catalog, before);
}
