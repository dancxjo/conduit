//! Safe fragmentation is exercised with selected text metrics, never fabricated rectangles.
use super::tests::{assert_geometry, item, prepared, run, wide};
use super::*;
use alloc::{format, vec};

#[test]
fn accessible_lanes_fill_residual_command_capacity_before_continuing() {
    let face = crate::native_face_scene::tests::face(7);
    let profile = prepared(&face, vec![wide(false)]);
    let items = (0..20)
        .map(|index| {
            let mut item = item(
                &format!("Distinct control {index}"),
                Some(usize::from(index >= 8)),
                true,
            );
            // Independent subjects permit accessible cuts between controls. The
            // source/control custody test separately projects actual domain Faces.
            item.subject_index = Some(index);
            item
        })
        .collect();
    let rows = run(items, 1200, 900, &profile).unwrap();
    assert_eq!(rows.len(), 20);
    assert_eq!(rows.iter().filter(|row| row.page == 0).count(), 19);
    assert_eq!(rows.iter().filter(|row| row.page == 1).count(), 1);
    let cause = rows.iter().find_map(|row| row.continuation).unwrap();
    assert!(cause.command_limit());
    assert!(!cause.row_limit() && !cause.block_limit());
    assert!(
        rows.iter()
            .filter(|row| row.page == 0)
            .all(|row| row.continuation.is_none())
    );
    assert_geometry(&rows, 1200, 900);
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.text, format!("Distinct control {index}"));
    }
}
#[test]
fn associated_field_cluster_that_fits_fresh_page_has_no_internal_cut() {
    let mut items = vec![];
    for index in 0..3 {
        let mut value = item("Prior ordinary reading", Some(0), false);
        value.subject_index = Some(index);
        items.push(value);
    }
    for (text, control) in [
        ("Bound field label", false),
        ("Current value", true),
        ("Associated help", false),
    ] {
        let mut value = item(text, Some(0), control);
        value.subject_index = Some(10);
        items.push(value);
    }
    let context = Context {
        items: &items,
        width: 500,
        scale: 100,
        padding: SPACE_SM,
        typography: Typography::EDITORIAL,
        rows: core::cell::Cell::new(0),
    };
    let mut fragments = leaves::arrange(&context, Some(0), 500, 180).unwrap();
    assert_eq!(fragments.len(), 2);
    assert_eq!(fragments[0].entries.len(), 3);
    assert_eq!(fragments[1].entries.len(), 3);
    let cause = fragments[1].continuation.unwrap();
    assert!(cause.block_limit() && cause.kept_group());
    assert!(fragments[1].take_prefix(80, 20, 40).is_none());
    assert_eq!(fragments[1].entries.len(), 3);
    let complete = fragments[1].take_prefix(180, 20, 40).unwrap();
    assert_eq!(complete.entries.len(), 3);
    assert!(fragments[1].entries.is_empty());
}

#[test]
fn aggregate_document_bytes_refuse_before_any_text_measurement() {
    let face = crate::native_face_scene::tests::face(7);
    let profile = prepared(&face, vec![wide(false)]);
    let segment = "x".repeat(MAX_DOCUMENT_BYTES / 2 + 1);
    let items = vec![
        item(&segment, Some(0), false),
        item(&segment, Some(1), false),
    ];
    assert_eq!(
        run(items, 1200, 900, &profile).err(),
        Some(FaceSceneError::DocumentBound)
    );
}

#[test]
fn column_continuation_reports_row_pressure_without_reordering_or_geometry_drift() {
    let face = crate::native_face_scene::tests::face(7);
    let profile = prepared(&face, vec![super::tests::narrow()]);
    let make = || {
        (0..21)
            .map(|index| {
                let mut value = item(&format!("Reading {index}"), Some(0), false);
                value.subject_index = Some(index);
                value
            })
            .collect()
    };
    let rows = run(make(), 1200, 2000, &profile).unwrap();
    let repeated = run(make(), 1200, 2000, &profile).unwrap();
    assert_eq!(rows.len(), 21);
    assert_geometry(&rows, 1200, 2000);
    for (index, (row, again)) in rows.iter().zip(&repeated).enumerate() {
        assert_eq!(row.text, format!("Reading {index}"));
        assert_eq!(
            (row.bounds, row.page, row.control, row.continuation),
            (again.bounds, again.page, again.control, again.continuation)
        );
    }
    let cause = rows.iter().find_map(|row| row.continuation).unwrap();
    assert!(cause.row_limit());
    assert!(!cause.block_limit() && !cause.command_limit());
}
