//! Durable admission keeps exact verified closure custody, not a public type list.
use conduit_core::*;
use conduit_plot::rust_binding::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

const INTERVAL: &str =
    "type Interval = {\n end: U64 in 0..=9\n start: U64 in 0..=9\n where .start <= .end\n}\n";
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 16384,
        scopes: 32,
        invariants: 32,
        contracts: 32,
    }
}
fn check(source: &str, base: &StartupCatalog) -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(source), base).unwrap()
}
fn local() -> NativeLawWitness {
    let source = format!("{INTERVAL}type Holder = {{\n value: Interval\n}}\n");
    check(&source, &StartupCatalog::new())
        .native_law_witness("Holder")
        .unwrap()
        .clone()
}
fn imported() -> NativeLawWitness {
    let owner = check(INTERVAL, &StartupCatalog::new());
    let mut base = StartupCatalog::new();
    base.insert_native_law_witness(
        "custody/Interval",
        owner.native_law_witness("Interval").unwrap(),
    )
    .unwrap();
    check(
        "with custody/Interval as Imported\ntype Holder = {\n value: Imported\n}\n",
        &base,
    )
    .native_law_witness("Holder")
    .unwrap()
    .clone()
}
fn family() -> NativeLawWitness {
    let manifest = "pack custody/admission (\n version = 1.0.0\n) {\n ship Boxed\n}\n";
    let source = "type Boxed<T> = {\n value: T\n}\n";
    let syntax = parse_syntax_document(manifest);
    let members = [PackageMemberSource {
        path: "main",
        source,
    }];
    let bundle =
        CheckedPackageBundle::from_sources(manifest, &syntax.packages[0], &members).unwrap();
    let exports =
        PackageExportCatalog::from_bundle(&bundle, manifest, &syntax.packages[0], &members)
            .unwrap();
    let mut base = StartupCatalog::new();
    exports.install_shipped_types(&mut base).unwrap();
    check(
        &format!(
            "with custody/admission/Boxed as Boxed\n{INTERVAL}type Holder = Boxed<Interval>\n"
        ),
        &base,
    )
    .native_law_witness("Holder")
    .unwrap()
    .clone()
}
fn bytes(witness: &NativeLawWitness, start: u64, end: u64) -> Vec<u8> {
    let root = &witness.checked_type().value_type;
    let StructuredInfoTypeShape::Record { fields, .. } = root.shape() else {
        panic!("root record")
    };
    let child = fields
        .iter()
        .find(|field| field.name() == "value")
        .unwrap()
        .value_type();
    let integer = |value: u64| {
        primitive_into_structured(
            StructuredInfoType::leaf(KindId::new("value/u64")).unwrap(),
            &value,
        )
        .unwrap()
    };
    let interval = StructuredInfoValue::record(
        child.clone(),
        vec![
            StructuredFieldValue::new("start", integer(start)).unwrap(),
            StructuredFieldValue::new("end", integer(end)).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::record(
        root.clone(),
        vec![StructuredFieldValue::new("value", interval).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn complete(owner: &mut OwnedPreparedNativeAdmission) -> Result<(), PreparedNativeRefusal> {
    loop {
        if owner.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
#[test]
fn local_imported_and_actual_package_family_keep_nested_laws_after_source_drop() {
    for witness in [local(), imported(), family()] {
        let good = bytes(&witness, 2, 7);
        let bad_law = bytes(&witness, 7, 2);
        let bad_contract = bytes(&witness, 1, 10);
        for (input, refusal) in [(good, 0), (bad_law, 1), (bad_contract, 2)] {
            let mut owner = OwnedPreparedNativeAdmission::new(&witness, bounds()).unwrap();
            owner.begin(&input).unwrap();
            let result = complete(&mut owner);
            match refusal {
                0 => {
                    result.unwrap();
                    let admitted = owner.admitted().unwrap();
                    assert_eq!(admitted.canonical_bytes(), input);
                    assert_eq!(admitted.native_type(), witness.checked_type());
                    assert_eq!(owner.native_type(), witness.checked_type());
                    assert_eq!(
                        admitted.source_witness().checked_type(),
                        witness.checked_type()
                    );
                    assert!(owner.actual_occurrences() >= 2);
                    assert!(owner.actual_contract_occurrences() >= 2);
                }
                1 => assert!(matches!(
                    result,
                    Err(PreparedNativeRefusal::Invariant { .. })
                )),
                _ => assert!(matches!(
                    result,
                    Err(PreparedNativeRefusal::Contract { .. })
                )),
            }
        }
    }
}
#[test]
fn begin_step_retire_reuse_and_failure_paths_do_not_allocate() {
    let witness = local();
    let good = bytes(&witness, 1, 3);
    let bad = bytes(&witness, 7, 2);
    let mut owner = OwnedPreparedNativeAdmission::new(&witness, bounds()).unwrap();
    drop(witness);
    let ((), observed) = allocation_probe::observe(|| {
        for _ in 0..3 {
            owner.begin(&good).unwrap();
            complete(&mut owner).unwrap();
            assert_eq!(owner.admitted().unwrap().canonical_bytes(), good);
            owner.retire_admitted().unwrap();
        }
        owner.begin(&bad).unwrap();
        assert!(matches!(
            complete(&mut owner),
            Err(PreparedNativeRefusal::Invariant { .. })
        ));
        assert!(owner.admitted().is_err());
        assert!(owner.retire_admitted().is_err());
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
}
#[test]
fn finite_pressure_and_cancel_never_expose_an_owned_witness() {
    let witness = local();
    let good = bytes(&witness, 1, 3);
    assert!(OwnedPreparedNativeAdmission::new(
        &witness,
        PreparedNativeBounds {
            scopes: 1,
            ..bounds()
        }
    )
    .is_err());
    let mut owner =
        OwnedPreparedNativeAdmission::new_with_occurrence_limit(&witness, bounds(), 1).unwrap();
    owner.begin(&good).unwrap();
    assert_eq!(complete(&mut owner), Err(PreparedNativeRefusal::Bounds));
    assert!(owner.admitted().is_err());
    let mut owner = OwnedPreparedNativeAdmission::new(&witness, bounds()).unwrap();
    let oversize = vec![0; bounds().canonical_bytes + 1];
    assert_eq!(owner.begin(&oversize), Err(PreparedNativeRefusal::Bounds));
    assert!(owner.retained_bytes().is_empty());
    owner.begin(&good).unwrap();
    let ((), observed) = allocation_probe::observe(|| {
        owner.cancel();
        assert_eq!(owner.step(), Err(PreparedNativeRefusal::Cancelled));
        assert!(owner.admitted().is_err());
        assert!(owner.retire_admitted().is_err());
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(owner.retained_bytes(), good);
}

#[test]
fn absent_optional_skips_nested_owners_without_stale_occurrence_or_allocation() {
    let doc = check(
        &format!("{INTERVAL}type Holder = {{\n value: Interval?\n}}\n"),
        &StartupCatalog::new(),
    );
    let witness = doc.native_law_witness("Holder").unwrap();
    let root = &witness.checked_type().value_type;
    let StructuredInfoTypeShape::Record { fields, .. } = root.shape() else {
        panic!()
    };
    let none = StructuredInfoValue::variant(
        fields[0].value_type().clone(),
        "none",
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(KindId::new(EMPTY_INFO_ID)).unwrap(),
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    let input = StructuredInfoValue::record(
        root.clone(),
        vec![StructuredFieldValue::new("value", none).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut owner = OwnedPreparedNativeAdmission::new(witness, bounds()).unwrap();
    drop(doc);
    let ((), observed) = allocation_probe::observe(|| {
        for _ in 0..2 {
            owner.begin(&input).unwrap();
            complete(&mut owner).unwrap();
            assert_eq!(owner.actual_occurrences(), 1);
            assert_eq!(owner.actual_contract_occurrences(), 0);
            assert_eq!(owner.admitted().unwrap().canonical_bytes(), input);
            owner.retire_admitted().unwrap();
            assert!(owner.invariant_output(1, 0).is_none());
            assert!(owner.latest_contract_occurrence().is_none());
        }
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
}
