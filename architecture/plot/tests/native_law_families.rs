//! Family replay uses original complete package Sources and independent anchors.
use conduit_plot::*;
const MANIFEST: &str = "pack custody/families (\n version = 1.0.0\n) {\n ship CustodyMatrix\n}\n";
const BOUND: &str = "type CustodyBound = U16 in 1..=64\n";
const MATRIX: &str =
    "type CustodyMatrix<N: CustodyBound> = {\n size: U16 in N..=N\n where .size == N\n}\n";
const CONSUMER: &str = "with custody/families/CustodyMatrix as Grid\ntype Value = Grid<32>\n";
fn owner() -> (StartupCatalog, [u8; 32]) {
    let manifest = parse_syntax_document(MANIFEST);
    let members = [
        PackageMemberSource {
            path: "bounds",
            source: BOUND,
        },
        PackageMemberSource {
            path: "matrix",
            source: MATRIX,
        },
    ];
    let bundle =
        CheckedPackageBundle::from_sources(MANIFEST, &manifest.packages[0], &members).unwrap();
    let digest = bundle.package.content_digest;
    let exports =
        PackageExportCatalog::from_bundle(&bundle, MANIFEST, &manifest.packages[0], &members)
            .unwrap();
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    (catalog, digest)
}
fn specimen() -> (NativeLawWitness, Vec<NativeFamilyAnchor>, NativeLawMaterial) {
    let (catalog, digest) = owner();
    let checked = check_syntax_document(&parse_syntax_document(CONSUMER), &catalog).unwrap();
    let witness = checked
        .native_law_witness("Value")
        .expect("actual instantiated family witness")
        .clone();
    let anchors = witness.family_anchors().unwrap();
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0].package_content_digest, digest);
    let material = witness.material().unwrap();
    (witness, anchors, material)
}
fn verify(
    witness: &NativeLawWitness,
    anchors: &[NativeFamilyAnchor],
    material: &NativeLawMaterial,
) -> Result<NativeLawWitness, NativeLawCustodyRefusal> {
    NativeLawWitness::verify_with_context(
        material,
        &witness.checked_type().value_type,
        &NativeLawContext { families: anchors },
    )
}
#[test]
fn actual_multimember_generic_package_replays_to_same_independent_type_and_laws() {
    let (witness, anchors, material) = specimen();
    let verified = verify(&witness, &anchors, &material).unwrap();
    assert_eq!(verified.checked_type(), witness.checked_type());
    assert_eq!(verified.family_anchors().unwrap(), anchors);
    assert!(matches!(
        NativeLawWitness::verify(&material, &witness.checked_type().value_type),
        Err(NativeLawCustodyRefusal::MissingContext)
    ));
    let family = &material.families[0].1;
    assert_eq!(family.manifest, MANIFEST);
    assert_eq!(family.members.len(), 2);
    assert_eq!(family.members[0].source, BOUND);
    assert_eq!(family.members[1].source, MATRIX);
    let (catalog, _) = owner();
    assert!(check_syntax_document(
        &parse_syntax_document(&CONSUMER.replace("Grid<32>", "Grid<0>")),
        &catalog
    )
    .is_err());
}
#[test]
fn missing_wrong_and_packet_self_derived_context_never_replace_independent_anchor() {
    let (witness, anchors, material) = specimen();
    assert!(verify(&witness, &[], &material).is_err());
    let mut wrong_digest = anchors.clone();
    wrong_digest[0].package_content_digest[0] ^= 1;
    assert!(verify(&witness, &wrong_digest, &material).is_err());
    let mut wrong_export = anchors.clone();
    wrong_export[0].exported_path.push_str("Foreign");
    assert!(verify(&witness, &wrong_export, &material).is_err());
    // A complete different bundle can retain exactly the same native meaning.
    // The original out-of-band package binding still rejects it.
    let mut foreign = material.clone();
    foreign.families[0].1.members[0]
        .source
        .push_str("# foreign publication\n");
    assert!(verify(&witness, &anchors, &foreign).is_err());
}
#[test]
fn missing_substituted_duplicate_reordered_and_renamed_members_refuse() {
    let (witness, anchors, original) = specimen();
    let mut omitted = original.clone();
    omitted.families[0].1.members.remove(0);
    assert!(verify(&witness, &anchors, &omitted).is_err());
    let mut substituted = original.clone();
    substituted.families[0].1.members[0].source = BOUND.replace("1..=64", "1..=63");
    assert!(verify(&witness, &anchors, &substituted).is_err());
    let mut missing_law = original.clone();
    missing_law.families[0].1.members[1].source = MATRIX.replace(" where .size == N\n", "");
    assert!(verify(&witness, &anchors, &missing_law).is_err());
    let mut duplicate = original.clone();
    let first = duplicate.families[0].1.members[0].clone();
    duplicate.families[0].1.members.push(first);
    assert!(verify(&witness, &anchors, &duplicate).is_err());
    let mut reordered = original.clone();
    reordered.families[0].1.members.swap(0, 1);
    assert!(verify(&witness, &anchors, &reordered).is_err());
    let mut renamed = original;
    renamed.families[0].1.members[0].path = "foreign-bounds".into();
    assert!(verify(&witness, &anchors, &renamed).is_err());
}
#[test]
fn application_and_claim_changes_do_not_gain_original_type_identity() {
    let (witness, anchors, original) = specimen();
    let mut changed_argument = original.clone();
    changed_argument.source = CONSUMER.replace("Grid<32>", "Grid<31>");
    assert!(verify(&witness, &anchors, &changed_argument).is_err());
    let mut omitted = original.clone();
    omitted.claimed.invariants.clear();
    assert!(verify(&witness, &anchors, &omitted).is_err());
    let mut changed_manifest = original;
    changed_manifest.families[0].1.manifest = MANIFEST.replace("1.0.0", "1.0.1");
    assert!(verify(&witness, &anchors, &changed_manifest).is_err());
}
#[test]
fn entire_family_payload_is_bounded_before_invalid_source_is_parsed() {
    let (witness, anchors, mut material) = specimen();
    material.source = "invalid root Source".into();
    material.families[0].1.members[0].source = " ".repeat(4 * 1024 * 1024 + 1);
    assert!(matches!(
        verify(&witness, &anchors, &material),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}

#[test]
fn mixed_family_graph_counts_each_manifest_and_member_source_before_parsing() {
    let (witness, anchors, mut material) = specimen();
    let original = material.families[0].1.clone();
    material.source = "deliberately invalid root Source".into();
    material.families.clear();
    // Three manifests and 300 tiny member Sources exceed256 Source-bearing
    // modules, while staying far below depth16 and aggregate4MiB.
    for family in 0..3 {
        let mut body = original.clone();
        body.members = (0..100)
            .map(|member| NativeFamilyMember {
                path: format!("member{member:03}"),
                source: "# retained tiny Source\n".into(),
            })
            .collect();
        material
            .families
            .push((format!("owner/family{family}"), body));
    }
    assert!(matches!(
        verify(&witness, &anchors, &material),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}

#[test]
fn nested_imported_family_replays_both_original_packages_with_both_anchors() {
    fn install(manifest: &str, source: &str, base: &mut StartupCatalog) {
        let parsed = parse_syntax_document(manifest);
        let sources = [PackageMemberSource {
            path: "main",
            source,
        }];
        let bundle =
            CheckedPackageBundle::from_sources(manifest, &parsed.packages[0], &sources).unwrap();
        let exports =
            PackageExportCatalog::from_bundle(&bundle, manifest, &parsed.packages[0], &sources)
                .unwrap();
        exports.install_shipped_types(base).unwrap();
    }
    let mut catalog = StartupCatalog::new();
    install(
        "pack custody/inner (\n version = 1.0.0\n) {\n ship CustodyInner\n}\n",
        "type CustodyInner<T> = {\n value: T\n}\n",
        &mut catalog,
    );
    install(
        "pack custody/outer (\n version = 1.0.0\n) {\n ship CustodyOuter\n}\n",
        "with custody/inner/CustodyInner as Imported\ntype CustodyOuter<T> = Imported<T>\n",
        &mut catalog,
    );
    let doc = check_syntax_document(
        &parse_syntax_document(
            "with custody/outer/CustodyOuter as Wrapped\ntype Value = Wrapped<U64>\n",
        ),
        &catalog,
    )
    .unwrap();
    let witness = doc
        .native_law_witness("Value")
        .expect("nested original family custody");
    let anchors = witness.family_anchors().unwrap();
    assert_eq!(anchors.len(), 2);
    let material = witness.material().unwrap();
    assert!(verify(witness, &anchors, &material).is_ok());
    let only_outer = anchors
        .iter()
        .filter(|anchor| anchor.exported_path.contains("/outer/"))
        .cloned()
        .collect::<Vec<_>>();
    assert!(verify(witness, &only_outer, &material).is_err());
    let mut tampered_inner = material;
    tampered_inner.families[0].1.families[0].1.members[0]
        .source
        .push_str("# different inner package\n");
    assert!(verify(witness, &anchors, &tampered_inner).is_err());
}

#[test]
fn optional_full_bundle_custody_does_not_consume_legacy_dependency_bytes() {
    for count in [2, 8] {
        let mut manifest = String::from("pack custody/padded (\n version = 1.0.0\n) {\n");
        let mut source = "# retained original padding\n".repeat(23_000);
        assert!(source.len() > 512 * 1024);
        assert!(source.len() < 1024 * 1024);
        for index in 0..count {
            manifest.push_str(&format!(" ship Padded{index}\n"));
            source.push_str(&format!("type Padded{index}<T> = {{\n value: T\n}}\n"));
        }
        manifest.push_str("}\n");
        let parsed = parse_syntax_document(&manifest);
        let members = [PackageMemberSource {
            path: "main",
            source: &source,
        }];
        let bundle =
            CheckedPackageBundle::from_sources(&manifest, &parsed.packages[0], &members).unwrap();
        let exports =
            PackageExportCatalog::from_bundle(&bundle, &manifest, &parsed.packages[0], &members)
                .unwrap();
        let mut catalog = StartupCatalog::new();
        // Two retained bundles already exceed the old 1MiB dependency ceiling;
        // eight exceed the independent 4MiB optional custody registry ceiling.
        exports.install_shipped_types(&mut catalog).unwrap();
        let first = check_syntax_document(
            &parse_syntax_document(
                "with custody/padded/Padded0 as Selected\ntype Value = Selected<U64>\n",
            ),
            &catalog,
        )
        .unwrap();
        if count == 2 {
            let witness = first
                .native_law_witness("Value")
                .expect("bounded optional custody");
            let anchors = witness.family_anchors().unwrap();
            assert!(verify(witness, &anchors, &witness.material().unwrap()).is_ok());
        } else {
            let last = check_syntax_document(
                &parse_syntax_document(
                    "with custody/padded/Padded7 as Selected\ntype Value = Selected<U64>\n",
                ),
                &catalog,
            )
            .unwrap();
            assert!(last.native_law_witness("Value").is_none());
            assert!(last.native_types.iter().any(|ty| ty.name == "Value"));
        }
    }
}
