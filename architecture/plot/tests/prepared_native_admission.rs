use conduit_core::*;
use conduit_plot::rust_binding::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn document() -> CheckedSyntaxDocument {
    check_syntax_document(
        &parse_syntax_document(
            r#"
type RawInterval = {
    end: U64
    start: U64
}
type Interval = {
    end: U64 in 0..=9
    start: U64 in 0..=9
    where .start <= .end
}
type Holder = {
    interval: Interval
}
type Other = {
    interval: Interval
}
"#,
        ),
        &StartupCatalog::new(),
    )
    .unwrap()
}
fn native<'a>(d: &'a CheckedSyntaxDocument, name: &str) -> &'a CheckedNativeType {
    d.native_types.iter().find(|v| v.name == name).unwrap()
}
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 16384,
        scopes: 32,
        invariants: 32,
    }
}
fn bytes(d: &CheckedSyntaxDocument, holder: &str, start: u64, end: u64) -> Vec<u8> {
    let integer = |n: u64| {
        primitive_into_structured(
            StructuredInfoType::leaf(KindId::new("value/u64")).unwrap(),
            &n,
        )
        .unwrap()
    };
    let interval = StructuredInfoValue::record(
        native(d, "Interval").value_type.clone(),
        vec![
            StructuredFieldValue::new("start", integer(start)).unwrap(),
            StructuredFieldValue::new("end", integer(end)).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::record(
        native(d, holder).value_type.clone(),
        vec![StructuredFieldValue::new("interval", interval).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn complete(admission: &mut PreparedNativeAdmission<'_>) -> Result<(), PreparedNativeRefusal> {
    loop {
        if admission.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
#[test]
fn nested_native_laws_and_contracts_are_required_without_runtime_allocation() {
    let d = document();
    let identity = &native(&d, "Holder").identity;
    let valid = bytes(&d, "Holder", 2, 7);
    let mut admission = PreparedNativeAdmission::new(&d, identity, bounds()).unwrap();
    let ((), observation) = allocation_probe::observe(|| {
        admission.begin(&valid).unwrap();
        complete(&mut admission).unwrap();
        let proof = admission.admitted().unwrap();
        assert!(core::ptr::eq(proof.source_document(), &d));
        assert_eq!(proof.canonical_bytes(), valid);
        assert_eq!(&proof.native_type().identity, identity);
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
    let invalid = bytes(&d, "Holder", 7, 2);
    let mut admission = PreparedNativeAdmission::new(&d, identity, bounds()).unwrap();
    admission.begin(&invalid).unwrap();
    assert!(matches!(
        complete(&mut admission),
        Err(PreparedNativeRefusal::Invariant { .. })
    ));
    assert!(admission.admitted().is_err());
    assert_eq!(admission.retained_bytes(), invalid);
    let invalid = bytes(&d, "Holder", 1, 10);
    let mut admission = PreparedNativeAdmission::new(&d, identity, bounds()).unwrap();
    admission.begin(&invalid).unwrap();
    assert!(matches!(
        complete(&mut admission),
        Err(PreparedNativeRefusal::Contract { .. })
    ));
    assert!(admission.admitted().is_err());
}
#[test]
fn foreign_schema_cancel_and_pressure_never_mint_native_truth() {
    let d = document();
    let identity = &native(&d, "Holder").identity;
    let valid = bytes(&d, "Holder", 1, 3);
    let mut admission = PreparedNativeAdmission::new(&d, identity, bounds()).unwrap();
    let foreign = bytes(&d, "Other", 1, 3);
    assert_eq!(admission.begin(&foreign), Err(PreparedNativeRefusal::Shape));
    assert_eq!(admission.retained_bytes(), foreign);
    assert!(admission.admitted().is_err());
    let mut admission = PreparedNativeAdmission::new(&d, identity, bounds()).unwrap();
    assert_eq!(
        admission.begin(&vec![0; bounds().canonical_bytes + 1]),
        Err(PreparedNativeRefusal::Bounds)
    );
    assert!(admission.retained_bytes().is_empty());
    admission.begin(&valid).unwrap();
    admission.cancel();
    assert_eq!(admission.step(), Err(PreparedNativeRefusal::Cancelled));
    assert_eq!(admission.retained_bytes(), valid);
    assert!(admission.admitted().is_err());
    assert!(PreparedNativeAdmission::new(
        &d,
        identity,
        PreparedNativeBounds {
            scopes: 1,
            ..bounds()
        }
    )
    .is_err());
}

#[test]
fn exact_field_composition_requires_laws_and_never_relabels_raw_bytes() {
    let d = document();
    let raw_type = native(&d, "RawInterval").value_type.clone();
    let target = native(&d, "Interval");
    let u64_type = StructuredInfoType::leaf(KindId::new("value/u64")).unwrap();
    let raw = StructuredInfoValue::record(
        raw_type,
        vec![
            StructuredFieldValue::new(
                "start",
                primitive_into_structured(u64_type.clone(), &7u64).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("end", primitive_into_structured(u64_type, &2u64).unwrap())
                .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut direct = PreparedNativeAdmission::new(&d, &target.identity, bounds()).unwrap();
    assert_eq!(direct.begin(&raw), Err(PreparedNativeRefusal::Shape));
    let mut composer =
        PreparedStructuredComposer::new(&target.value_type, bounds().canonical_bytes).unwrap();
    let mut admission = PreparedNativeAdmission::new(&d, &target.identity, bounds()).unwrap();
    let (result, observation) = allocation_probe::observe(|| {
        let raw = validate_canonical_structured_value(&raw).unwrap();
        let end = raw.record_field("end").unwrap().unwrap();
        let start = raw.record_field("start").unwrap().unwrap();
        let exact = composer.record(&[end, start]).unwrap();
        admission.begin(exact).unwrap();
        complete(&mut admission)
    });
    assert!(matches!(
        result,
        Err(PreparedNativeRefusal::Invariant { .. })
    ));
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
    assert!(admission.admitted().is_err());
}

#[test]
fn public_inspection_metadata_cannot_supply_or_weaken_checked_authority() {
    let mut d = document();
    let identity = native(&d, "Interval").identity.clone();
    let mut forged = native(&d, "Interval").clone();
    forged.identity = KindId::new("forged/fresh-native");
    d.native_types.push(forged.clone());
    assert!(matches!(
        PreparedNativeAdmission::new(&d, &forged.identity, bounds()),
        Err(PreparedNativeRefusal::MissingNativeMetadata)
    ));
    let changed = d
        .native_types
        .iter_mut()
        .find(|v| v.identity == identity)
        .unwrap();
    changed.invariants.clear();
    changed.value_contracts.clear();
    assert!(matches!(
        PreparedNativeAdmission::new(&d, &identity, bounds()),
        Err(PreparedNativeRefusal::ConflictingNativeMetadata)
    ));
    // Removing public inspection data leaves the exact private checked owner.
    d.native_types.clear();
    let admission = PreparedNativeAdmission::new(&d, &identity, bounds()).unwrap();
    assert_eq!(admission.native_type().identity, identity);
    assert!(!admission.native_type().invariants.is_empty());
}

#[test]
fn preparation_reports_granted_limits_and_actual_storage_without_runtime_growth() {
    let d = document();
    let mut admission =
        PreparedNativeAdmission::new(&d, &native(&d, "Holder").identity, bounds()).unwrap();
    let report = admission.capacity_report();
    assert_eq!(report.granted, bounds());
    assert_eq!(report.prepared_scopes, 2);
    assert_eq!(report.prepared_evaluators, 1);
    assert_eq!(report.occurrence_limit, MAXIMUM_PREPARED_NATIVE_OCCURRENCES);
    assert!(report.canonical_input_capacity >= bounds().canonical_bytes);
    assert!(report.canonical_scratch_capacity >= bounds().canonical_bytes);
    assert!(report.evaluator_output_capacity > 0);
    let bytes = bytes(&d, "Holder", 2, 7);
    let ((), observation) = allocation_probe::observe(|| {
        admission.begin(&bytes).unwrap();
        complete(&mut admission).unwrap();
        assert_eq!(admission.capacity_report(), report);
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
}

#[test]
fn checked_nested_nominal_aliases_execute_inner_and_outer_laws_without_growth() {
    let d = check_syntax_document(
        &parse_syntax_document("type Inner = U64 where . >= 2\ntype Outer = Inner in 0..=7\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let inner = native(&d, "Inner");
    let outer = native(&d, "Outer");
    let StructuredInfoTypeShape::Nominal { representation, .. } = outer.value_type.shape() else {
        panic!()
    };
    assert_eq!(representation, &inner.value_type);
    for (number, expected_failure) in [(3u64, None), (1, Some(inner)), (9, Some(outer))] {
        let bytes = StructuredInfoValue::nominal(
            outer.value_type.clone(),
            StructuredInfoValue::nominal(
                inner.value_type.clone(),
                primitive_into_structured(
                    StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                    &number,
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let mut admission = PreparedNativeAdmission::new(&d, &outer.identity, bounds()).unwrap();
        assert_eq!(admission.scope_count(), 2);
        assert_eq!(admission.capacity_report().prepared_evaluators, 1);
        let (result, observation) = allocation_probe::observe(|| {
            admission.begin(&bytes).unwrap();
            complete(&mut admission)
        });
        assert_eq!(observation.allocations, 0);
        assert_eq!(observation.reallocations, 0);
        if let Some(expected) = expected_failure {
            let scope = match result.unwrap_err() {
                PreparedNativeRefusal::Invariant {
                    scope,
                    invariant: 0,
                } if number == 1 => {
                    assert_eq!(admission.invariant_output(scope, 0), Some([0u8].as_slice()));
                    scope
                }
                PreparedNativeRefusal::Contract {
                    scope, contract: 0, ..
                } if number == 9 => scope,
                reason => panic!("wrong refusal: {reason:?}"),
            };
            assert_eq!(
                admission.scope_native_type(scope).unwrap().identity,
                expected.identity
            );
            assert!(admission.admitted().is_err());
        } else {
            result.unwrap();
            let witness = admission.admitted().unwrap();
            assert!(core::ptr::eq(witness.source_document(), &d));
            assert_eq!(witness.canonical_bytes(), bytes);
            assert_eq!(admission.actual_occurrences(), 2);
        }
        assert_eq!(admission.retained_bytes(), bytes);
    }
}
