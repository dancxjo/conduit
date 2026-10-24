//! Wire transport preserves claims; only independent Source replay grants custody.
use conduit_plot::*;
const SOURCE: &str = "type Positive = U64 in 1..=65535\ntype Timing = {\n start: Positive\n duration: Positive\n where .start + .duration <= 65535\n}\n";
fn specimen() -> NativeLawWitness {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    checked.native_law_witness("Timing").unwrap().clone()
}
#[test]
fn actual_import_closure_round_trips_and_duplicate_imports_refuse_encoding() {
    let owner = specimen();
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_native_law_witness("wire/Timing", &owner)
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "with wire/Timing as Imported\ntype Holder = {\n value: Imported\n}\n",
        ),
        &catalog,
    )
    .unwrap();
    let witness = checked.native_law_witness("Holder").unwrap();
    let mut original = witness.material().unwrap();
    assert_eq!(original.imports.len(), 1);
    let decoded =
        NativeLawMaterial::from_canonical_bytes(&original.canonical_bytes().unwrap()).unwrap();
    assert_eq!(decoded, original);
    assert!(NativeLawWitness::verify(&decoded, &witness.checked_type().value_type).is_ok());
    original.imports.push(original.imports[0].clone());
    assert!(matches!(
        original.canonical_bytes(),
        Err(NativeLawCustodyRefusal::DuplicateImport)
    ));
}
#[test]
fn canonical_claim_round_trip_preserves_exact_type_contracts_and_original_predicate_bytes() {
    let witness = specimen();
    let original = witness.material().unwrap();
    let bytes = original.canonical_bytes().unwrap();
    let decoded = NativeLawMaterial::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(decoded.canonical_bytes().unwrap(), bytes);
    let verified = NativeLawWitness::verify(&decoded, &witness.checked_type().value_type).unwrap();
    assert_eq!(verified.checked_type(), witness.checked_type());
    // A well-framed modified claim still has no authority, even if re-encoded.
    let mut omitted = original;
    omitted.claimed.invariants.clear();
    let decoded =
        NativeLawMaterial::from_canonical_bytes(&omitted.canonical_bytes().unwrap()).unwrap();
    assert!(NativeLawWitness::verify(&decoded, &witness.checked_type().value_type).is_err());
}
#[test]
fn malformed_truncated_trailing_and_excessive_wire_material_refuse() {
    let original = specimen().material().unwrap().canonical_bytes().unwrap();
    for length in 0..original.len() {
        assert!(
            NativeLawMaterial::from_canonical_bytes(&original[..length]).is_err(),
            "prefix {length}"
        );
    }
    let mut unknown = original.clone();
    unknown[0] ^= 1;
    assert!(NativeLawMaterial::from_canonical_bytes(&unknown).is_err());
    let mut trailing = original.clone();
    trailing.push(0);
    assert!(NativeLawMaterial::from_canonical_bytes(&trailing).is_err());
    let mut count = original;
    let header = b"conduit.native-law-material.v1\0".len();
    count[header..header + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        NativeLawMaterial::from_canonical_bytes(&count),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
    assert!(matches!(
        NativeLawMaterial::from_canonical_bytes(&vec![0; 4 * 1024 * 1024 + 1]),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}

#[test]
fn raw_depth_and_aggregate_document_limits_refuse_before_decoding_claims() {
    // Deliberately empty Type frames would fail Type decoding. Capacity must
    // instead be observed by the complete raw framing pass, before that phase.
    fn node(children: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = vec![0; 7 * 4]; // five fields, contracts, invariants
        bytes.extend_from_slice(&(children.len() as u32).to_le_bytes());
        for child in children {
            bytes.extend_from_slice(&0u32.to_le_bytes()); // import name
            bytes.extend_from_slice(child);
        }
        bytes.extend_from_slice(&0u32.to_le_bytes()); // families
        bytes
    }
    fn wire(body: &[u8]) -> Vec<u8> {
        let mut bytes = b"conduit.native-law-material.v1\0".to_vec();
        bytes.extend_from_slice(body);
        bytes
    }
    let mut deep = node(&[]);
    for _ in 0..17 {
        deep = node(&[deep]);
    }
    assert!(matches!(
        NativeLawMaterial::from_canonical_bytes(&wire(&deep)),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
    let wide = node(&vec![node(&[]); 256]);
    assert!(matches!(
        NativeLawMaterial::from_canonical_bytes(&wire(&wide)),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}
#[test]
fn complete_package_claim_keeps_original_members_but_cannot_supply_its_own_anchor() {
    let manifest = "pack wire/family (\n version = 1.0.0\n) {\n ship Box\n}\n";
    let source = "type Box<T> = {\n value: T\n}\n";
    let syntax = parse_syntax_document(manifest);
    let members = [PackageMemberSource {
        path: "box",
        source,
    }];
    let bundle =
        CheckedPackageBundle::from_sources(manifest, &syntax.packages[0], &members).unwrap();
    let exports =
        PackageExportCatalog::from_bundle(&bundle, manifest, &syntax.packages[0], &members)
            .unwrap();
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document("with wire/family/Box as Imported\ntype Value = Imported<U64>\n"),
        &catalog,
    )
    .unwrap();
    let witness = checked.native_law_witness("Value").unwrap();
    let anchors = witness.family_anchors().unwrap();
    let original = witness.material().unwrap();
    let decoded =
        NativeLawMaterial::from_canonical_bytes(&original.canonical_bytes().unwrap()).unwrap();
    assert_eq!(decoded, original);
    assert!(NativeLawWitness::verify(&decoded, &witness.checked_type().value_type).is_err());
    assert!(NativeLawWitness::verify_with_context(
        &decoded,
        &witness.checked_type().value_type,
        &NativeLawContext { families: &anchors }
    )
    .is_ok());
    let mut foreign = anchors;
    foreign[0].package_content_digest[0] ^= 1;
    assert!(NativeLawWitness::verify_with_context(
        &decoded,
        &witness.checked_type().value_type,
        &NativeLawContext { families: &foreign }
    )
    .is_err());
}
