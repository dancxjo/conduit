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
    assert!(fragments[1].take_prefix(80, 20, 40).is_none());
    assert_eq!(fragments[1].entries.len(), 3);
    let complete = fragments[1].take_prefix(180, 20, 40).unwrap();
    assert_eq!(complete.entries.len(), 3);
    assert!(fragments[1].entries.is_empty());
}
