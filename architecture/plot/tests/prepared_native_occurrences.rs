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
