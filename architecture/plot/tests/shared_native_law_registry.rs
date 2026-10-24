//! Shared metadata originates only in actual opaque checked Source custody.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn witness() -> NativeLawWitness {
    check_syntax_document_with_native_law_roots(&parse_syntax_document(
        "type Interval = {\n start: U64 in 0..=9\n end: U64 in 0..=9\n where .start <= .end\n}\ntype Holder = {\n value: Interval\n}\n"),
        &StartupCatalog::new(),&["Holder"],
    ).unwrap().native_law_witness("Holder").unwrap().clone()
}
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 16384,
        scopes: 32,
        invariants: 32,
        contracts: 32,
    }
}
fn bytes(ty: &StructuredInfoType, start: u64, end: u64) -> Vec<u8> {
    let scalar = |n| {
        primitive_into_structured(StructuredInfoType::leaf(kind_id("value/u64")).unwrap(), &n)
            .unwrap()
    };
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    let value = StructuredInfoValue::record(
        fields[0].value_type().clone(),
        vec![
            StructuredFieldValue::new("start", scalar(start)).unwrap(),
            StructuredFieldValue::new("end", scalar(end)).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new("value", value).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn complete(guard: &mut PreparedRegisteredNativeAdmission) -> Result<(), PreparedNativeRefusal> {
    loop {
        if guard.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
#[test]
fn one_registry_prepares_independent_native_guards_after_original_source_drop() {
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<NativeLawRegistry>();
    require_send_sync::<PreparedRegisteredNativeAdmission>();
    let witness = witness();
    let kind = witness.checked_type().identity.clone();
    let ty = witness.checked_type().value_type.clone();
    let registry = NativeLawRegistry::new(core::slice::from_ref(&witness)).unwrap();
    assert_eq!(registry.source_witnesses(), core::slice::from_ref(&witness));
    let mut first = registry.prepare_native(&kind, bounds()).unwrap();
    let mut second = registry.prepare_native(&kind, bounds()).unwrap();
    drop(registry);
    drop(witness);
    let good = bytes(&ty, 2, 7);
    let bad = bytes(&ty, 7, 2);
    first.begin(&good).unwrap();
    complete(&mut first).unwrap();
    let admitted = first.admitted().unwrap();
    assert_eq!(admitted.canonical_bytes(), good);
    assert_eq!(admitted.native_type().value_type, ty);
    assert_eq!(admitted.source_registry().source_witnesses().len(), 1);
    second.begin(&bad).unwrap();
    assert!(matches!(
        complete(&mut second),
        Err(PreparedNativeRefusal::Invariant { .. })
    ));
    assert!(second.admitted().is_err());
}
#[test]
fn successful_reuse_refusal_and_cancellation_have_no_play_allocations() {
    let witness = witness();
    let good = bytes(&witness.checked_type().value_type, 2, 7);
    let bad = bytes(&witness.checked_type().value_type, 7, 2);
    let registry = NativeLawRegistry::new(core::slice::from_ref(&witness)).unwrap();
    let mut guard = registry
        .prepare_native(&witness.checked_type().identity, bounds())
        .unwrap();
    let ((), observed) = allocation_probe::observe(|| {
        for _ in 0..2 {
            guard.begin(&good).unwrap();
            complete(&mut guard).unwrap();
            assert_eq!(guard.admitted().unwrap().canonical_bytes(), good);
            guard.retire_admitted().unwrap();
        }
        guard.begin(&bad).unwrap();
        assert!(complete(&mut guard).is_err());
        assert!(guard.admitted().is_err());
        assert!(guard.retire_admitted().is_err());
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    let mut guard = registry
        .prepare_native(&witness.checked_type().identity, bounds())
        .unwrap();
    guard.begin(&good).unwrap();
    let ((), observed) = allocation_probe::observe(|| {
        guard.cancel();
        assert_eq!(guard.step(), Err(PreparedNativeRefusal::Cancelled));
        assert!(guard.admitted().is_err());
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
}
#[test]
fn aggregate_source_pressure_unknown_identity_and_guard_pressure_are_honest_refusals() {
    let witness = witness();
    let repeated = vec![witness.clone(); 257];
    assert!(matches!(
        NativeLawRegistry::new(&repeated),
        Err(PreparedNativeRefusal::Bounds)
    ));
    let registry = NativeLawRegistry::new(core::slice::from_ref(&witness)).unwrap();
    assert!(matches!(
        registry.prepare_native(&kind_id("type/foreign"), bounds()),
        Err(PreparedNativeRefusal::MissingNativeMetadata)
    ));
    assert!(matches!(
        registry.prepare_native(
            &witness.checked_type().identity,
            PreparedNativeBounds {
                scopes: 1,
                ..bounds()
            }
        ),
        Err(PreparedNativeRefusal::Bounds)
    ));
    let good = bytes(&witness.checked_type().value_type, 1, 3);
    let mut guard = registry
        .prepare_native_with_occurrence_limit(&witness.checked_type().identity, bounds(), 1)
        .unwrap();
    guard.begin(&good).unwrap();
    assert_eq!(complete(&mut guard), Err(PreparedNativeRefusal::Bounds));
    assert!(guard.admitted().is_err());
}

#[test]
fn actual_imported_and_package_generic_owners_keep_nested_registry_custody() {
    let source =
        "type Interval = {\n start: U64 in 0..=9\n end: U64 in 0..=9\n where .start <= .end\n}\n";
    let checked = check_syntax_document_with_native_law_roots(
        &parse_syntax_document(source),
        &StartupCatalog::new(),
        &["Interval"],
    )
    .unwrap();
    let interval = checked.native_law_witness("Interval").unwrap();
    let mut imported = StartupCatalog::new();
    imported
        .insert_native_law_witness("owner/Interval", interval)
        .unwrap();
    let direct = check_syntax_document_with_native_law_roots(
        &parse_syntax_document(
            "with owner/Interval as Imported\ntype Holder = {\n value: Imported\n}\n",
        ),
        &imported,
        &["Holder"],
    )
    .unwrap();
    let manifest = "pack registry/family (\n version = 1.0.0\n) {\n ship Boxed\n}\n";
    let parsed = parse_syntax_document(manifest);
    let members = [PackageMemberSource {
        path: "main",
        source: "type Boxed<T> = {\n value: T\n}\n",
    }];
    let bundle =
        CheckedPackageBundle::from_sources(manifest, &parsed.packages[0], &members).unwrap();
    let exports =
        PackageExportCatalog::from_bundle(&bundle, manifest, &parsed.packages[0], &members)
            .unwrap();
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    let generic = check_syntax_document_with_native_law_roots(
        &parse_syntax_document(&format!(
            "with registry/family/Boxed as Boxed\n{source}type Holder = Boxed<Interval>\n"
        )),
        &catalog,
        &["Holder"],
    )
    .unwrap();
    assert_eq!(
        generic
            .native_law_witness("Holder")
            .unwrap()
            .family_anchors()
            .unwrap()
            .len(),
        1
    );
    for owner in [&direct, &generic] {
        let witness = owner.native_law_witness("Holder").unwrap();
        let registry = NativeLawRegistry::new(core::slice::from_ref(witness)).unwrap();
        let mut guard = registry
            .prepare_native(&witness.checked_type().identity, bounds())
            .unwrap();
        let good = bytes(&witness.checked_type().value_type, 2, 7);
        guard.begin(&good).unwrap();
        complete(&mut guard).unwrap();
        assert_eq!(guard.admitted().unwrap().canonical_bytes(), good);
        let mut guard = registry
            .prepare_native(&witness.checked_type().identity, bounds())
            .unwrap();
        guard
            .begin(&bytes(&witness.checked_type().value_type, 7, 2))
            .unwrap();
        assert!(matches!(
            complete(&mut guard),
            Err(PreparedNativeRefusal::Invariant { .. })
        ));
    }
}
#[test]
fn aggregate_source_bytes_are_bounded_even_when_each_opaque_root_fits() {
    let mut source = String::from("type Cell = {\n");
    for index in 0..63 {
        source.push_str(&format!(" f{index:03}: U64\n"));
    }
    source.push_str("}\ntype Large = {\n");
    for index in 0..63 {
        source.push_str(&format!(" f{index:03}: Cell\n"));
    }
    for index in 0..8 {
        source.push_str(&format!(" where .f000.f000 <= {}\n", index + 1));
    }
    source.push_str("}\n");
    let checked = check_syntax_document_with_native_law_roots(
        &parse_syntax_document(&source),
        &StartupCatalog::new(),
        &["Large"],
    )
    .unwrap();
    let witness = checked.native_law_witness("Large").unwrap();
    assert!(witness.material().is_ok());
    assert!(NativeLawRegistry::new(core::slice::from_ref(witness)).is_ok());
    assert!(matches!(
        NativeLawRegistry::new(&[witness.clone(), witness.clone()]),
        Err(PreparedNativeRefusal::Bounds)
    ));
}
