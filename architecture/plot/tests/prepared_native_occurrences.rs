use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn document() -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document("type Interval = {\n start: U64\n end: U64\n where .start <= .end\n}\ntype Row = {\n cells: sequence Interval? <= 64\n}\ntype Batch = {\n rows: sequence Row <= 128\n}\n"),&StartupCatalog::new()).unwrap()
}
fn native<'a>(doc: &'a CheckedSyntaxDocument, name: &str) -> &'a CheckedNativeType {
    doc.native_types.iter().find(|v| v.name == name).unwrap()
}
fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn element(ty: &StructuredInfoType) -> StructuredInfoType {
    let StructuredInfoTypeShape::Sequence { element, .. } = ty.shape() else {
        panic!()
    };
    element.clone()
}
fn bytes(doc: &CheckedSyntaxDocument, rows: &[&[Option<(u64, u64)>]]) -> Vec<u8> {
    let batch_type = native(doc, "Batch").value_type.clone();
    let rows_type = field_type(&batch_type, "rows");
    let row_type = element(&rows_type);
    let cells_type = field_type(&row_type, "cells");
    let optional = element(&cells_type);
    let scalar = |v: u64| {
        primitive_into_structured(StructuredInfoType::leaf(kind_id("value/u64")).unwrap(), &v)
            .unwrap()
    };
    let rows = rows
        .iter()
        .map(|row| {
            let cells = row
                .iter()
                .map(|cell| {
                    if let Some((start, end)) = cell {
                        let interval = StructuredInfoValue::record(
                            native(doc, "Interval").value_type.clone(),
                            vec![
                                StructuredFieldValue::new("start", scalar(*start)).unwrap(),
                                StructuredFieldValue::new("end", scalar(*end)).unwrap(),
                            ],
                        )
                        .unwrap();
                        StructuredInfoValue::variant(optional.clone(), "some", interval).unwrap()
                    } else {
                        StructuredInfoValue::variant(
                            optional.clone(),
                            "none",
                            StructuredInfoValue::leaf(
                                StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
                                vec![],
                            )
                            .unwrap(),
                        )
                        .unwrap()
                    }
                })
                .collect();
            StructuredInfoValue::record(
                row_type.clone(),
                vec![StructuredFieldValue::new(
                    "cells",
                    StructuredInfoValue::sequence(cells_type.clone(), cells).unwrap(),
                )
                .unwrap()],
            )
            .unwrap()
        })
        .collect();
    StructuredInfoValue::record(
        batch_type,
        vec![StructuredFieldValue::new(
            "rows",
            StructuredInfoValue::sequence(rows_type, rows).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 16384,
        scopes: 64,
        invariants: 64,
        contracts: 64,
    }
}
fn run(
    admission: &mut PreparedNativeAdmission<'_>,
    input: &[u8],
) -> Result<(), PreparedNativeRefusal> {
    admission.begin(input)?;
    loop {
        if admission.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
fn actual_indices(admission: &PreparedNativeAdmission<'_>) -> Vec<u16> {
    let occurrence = admission.latest_occurrence().unwrap();
    occurrence
        .path
        .iter()
        .zip(occurrence.indices)
        .filter_map(|(step, index)| matches!(step, NativePathStep::Each).then_some(*index))
        .collect()
}
#[test]
fn huge_declared_cartesian_shape_prepares_once_and_visits_only_actual_some_payloads() {
    let doc = document();
    let identity = &native(&doc, "Batch").identity;
    let mut admission = PreparedNativeAdmission::new(&doc, identity, bounds()).unwrap();
    // Declared 128*64 optional cells far exceed the old expansion profile.
    // Three law templates suffice: Batch, Row and present Interval.
    assert_eq!(admission.scope_count(), 3);
    let input = bytes(&doc, &[&[], &[None, Some((2, 7))], &[Some((1, 1)), None]]);
    let (result, allocations) = allocation_probe::observe(|| run(&mut admission, &input));
    result.unwrap();
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(admission.actual_occurrences(), 6); // root + three rows + two intervals
    assert_eq!(admission.admitted().unwrap().canonical_bytes(), input);
    assert_eq!(actual_indices(&admission), vec![2, 0]);
    let occurrence = admission.latest_occurrence().unwrap();
    assert_eq!(
        admission.invariant_output(occurrence.scope, 0),
        Some([1u8].as_slice())
    );
    admission.retire_admitted().unwrap();
    assert!(admission.latest_occurrence().is_none());
    // Reuse resets traversal and suppresses stale outputs from absent payloads.
    let empty = bytes(&doc, &[&[None], &[]]);
    run(&mut admission, &empty).unwrap();
    assert_eq!(admission.actual_occurrences(), 3);
    let interval_scope = (0..admission.scope_count())
        .find(|i| admission.scope_native_type(*i).unwrap().name == "Interval")
        .unwrap();
    assert!(admission.invariant_output(interval_scope, 0).is_none());
}
#[test]
fn later_nested_occurrence_failure_retains_exact_indices_and_latest_false_source_output() {
    let doc = document();
    let mut admission =
        PreparedNativeAdmission::new(&doc, &native(&doc, "Batch").identity, bounds()).unwrap();
    let input = bytes(&doc, &[&[Some((1, 2)), None], &[None, Some((9, 3))]]);
    let (result, allocations) = allocation_probe::observe(|| run(&mut admission, &input));
    let PreparedNativeRefusal::Invariant { scope, invariant } = result.unwrap_err() else {
        panic!()
    };
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(actual_indices(&admission), vec![1, 1]);
    assert_eq!(admission.scope_native_type(scope).unwrap().name, "Interval");
    assert_eq!(
        admission.invariant_output(scope, invariant),
        Some([0u8].as_slice())
    );
    assert_eq!(admission.retained_bytes(), input);
    assert!(admission.admitted().is_err());
    assert_eq!(
        admission.step(),
        Err(PreparedNativeRefusal::Invariant { scope, invariant })
    );
}
#[test]
fn actual_occurrence_budget_exhaustion_never_truncates_or_relabels_previous_output() {
    let doc = document();
    let input = bytes(&doc, &[&[Some((1, 2))], &[Some((3, 4))]]);
    let mut admission = PreparedNativeAdmission::new_with_occurrence_limit(
        &doc,
        &native(&doc, "Batch").identity,
        bounds(),
        4,
    )
    .unwrap();
    let (result, allocations) = allocation_probe::observe(|| run(&mut admission, &input));
    assert_eq!(result, Err(PreparedNativeRefusal::Bounds));
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(admission.actual_occurrences(), 4);
    assert_eq!(actual_indices(&admission), vec![1, 0]);
    let occurrence = admission.latest_occurrence().unwrap();
    assert_eq!(occurrence.checked_invariants, 0);
    assert!(admission.invariant_output(occurrence.scope, 0).is_none());
    assert!(admission.admitted().is_err());
    assert_eq!(admission.retained_bytes(), input);
}

#[test]
fn later_primitive_contract_failure_retains_exact_collection_occurrence() {
    let doc = refined_document();
    let input = bytes(&doc, &[&[Some((1, 2)), None], &[None, Some((2, 10))]]);
    let mut admission =
        PreparedNativeAdmission::new(&doc, &native(&doc, "Batch").identity, bounds()).unwrap();
    let (result, observation) = allocation_probe::observe(|| run(&mut admission, &input));
    let reason = result.unwrap_err();
    assert!(matches!(reason, PreparedNativeRefusal::Contract { .. }));
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
    assert_eq!(observation.deallocations, 0);
    let occurrence = admission.latest_contract_occurrence().unwrap();
    let indices: Vec<_> = occurrence
        .path
        .iter()
        .zip(occurrence.indices)
        .filter_map(|(part, index)| matches!(part, NativePathStep::Each).then_some(*index))
        .collect();
    assert_eq!(indices, vec![1, 1]);
    assert!(occurrence.checked);
    let source = &admission
        .scope_native_type(occurrence.scope)
        .unwrap()
        .value_contracts[occurrence.contract];
    assert_eq!(source.representation_path, ".rows[].cells[]?some.end");
    assert_eq!(admission.retained_bytes(), input);
    assert!(admission.admitted().is_err());
    assert_eq!(admission.step(), Err(reason));
}

fn refined_document() -> CheckedSyntaxDocument {
    check_syntax_document(
        &parse_syntax_document("type Interval = {\n start: U64 in 0..=9\n end: U64 in 0..=9\n where .start <= .end\n}\ntype Row = {\n cells: sequence Interval? <= 64\n}\ntype Batch = {\n rows: sequence Row <= 128\n}\n"),
        &StartupCatalog::new(),
    ).unwrap()
}
#[test]
fn contract_work_is_cooperative_bounded_cancellable_and_reusable_without_heap_activity() {
    let doc = refined_document();
    let identity = &native(&doc, "Batch").identity;
    let input = bytes(&doc, &[&[Some((1, 2)), None], &[None, Some((2, 9))]]);
    let mut admission = PreparedNativeAdmission::new(&doc, identity, bounds()).unwrap();
    let report = admission.capacity_report();
    assert_eq!(report.prepared_contracts, 6);
    assert!(report.contract_index_capacity > 0);
    let (result, observation) = allocation_probe::observe(|| {
        admission.begin(&input)?;
        loop {
            let before = admission.actual_contract_occurrences();
            let progress = admission.step()?;
            assert!(admission.actual_contract_occurrences() - before <= 1);
            if progress == PreparedNativeProgress::Admitted {
                break;
            }
        }
        Ok::<_, PreparedNativeRefusal>(())
    });
    result.unwrap();
    assert_eq!(
        (
            observation.allocations,
            observation.reallocations,
            observation.deallocations
        ),
        (0, 0, 0)
    );
    assert_eq!(admission.actual_contract_occurrences(), 12);
    assert_eq!(admission.actual_occurrences(), 5);
    assert_eq!(admission.capacity_report(), report);
    admission.retire_admitted().unwrap();
    assert!(admission.latest_contract_occurrence().is_none());
    let absent = bytes(&doc, &[&[None], &[]]);
    run(&mut admission, &absent).unwrap();
    assert_eq!(admission.actual_contract_occurrences(), 0);
    assert!(admission.latest_contract_occurrence().is_none());
    assert_eq!(admission.capacity_report(), report);

    let mut limited =
        PreparedNativeAdmission::new_with_occurrence_limit(&doc, identity, bounds(), 1).unwrap();
    let (result, observation) = allocation_probe::observe(|| run(&mut limited, &input));
    assert_eq!(result, Err(PreparedNativeRefusal::Bounds));
    assert_eq!(
        (
            observation.allocations,
            observation.reallocations,
            observation.deallocations
        ),
        (0, 0, 0)
    );
    assert_eq!(limited.actual_contract_occurrences(), 1);
    assert_eq!(limited.actual_occurrences(), 0);
    assert!(!limited.latest_contract_occurrence().unwrap().checked);
    assert_eq!(limited.retained_bytes(), input);
    limited.cancel();
    assert_eq!(limited.step(), Err(PreparedNativeRefusal::Bounds));
    assert!(limited.admitted().is_err());

    let mut cancelled = PreparedNativeAdmission::new(&doc, identity, bounds()).unwrap();
    let (result, observation) = allocation_probe::observe(|| {
        cancelled.begin(&input).unwrap();
        assert_eq!(cancelled.step().unwrap(), PreparedNativeProgress::Pending);
        assert_eq!(cancelled.actual_contract_occurrences(), 1);
        cancelled.cancel();
        cancelled.step()
    });
    assert_eq!(result, Err(PreparedNativeRefusal::Cancelled));
    assert_eq!(
        (
            observation.allocations,
            observation.reallocations,
            observation.deallocations
        ),
        (0, 0, 0)
    );
    assert_eq!(cancelled.actual_contract_occurrences(), 1);
    assert!(cancelled.latest_contract_occurrence().unwrap().checked);
    assert_eq!(cancelled.retained_bytes(), input);
    assert!(cancelled.admitted().is_err());
    assert!(PreparedNativeAdmission::new(
        &doc,
        identity,
        PreparedNativeBounds {
            contracts: 5,
            ..bounds()
        }
    )
    .is_err());
}
