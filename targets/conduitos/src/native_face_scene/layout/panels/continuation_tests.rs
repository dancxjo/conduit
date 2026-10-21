//! CPU regression specifications; no installed-product or input proof.
use super::{
    tests::{assert_disjoint, item, screen},
    *,
};
use alloc::{format, vec};

#[test]
fn sole_populated_region_keeps_full_width_on_every_page() {
    let inputs = (0..24)
        .map(|index| item(&format!("Reading {index}"), Some(1)))
        .collect::<Vec<_>>();
    let expected = inputs
        .iter()
        .map(|item| item.text.as_str())
        .collect::<String>();
    let rows = admit_configured(
        inputs,
        screen(1280, 800),
        12,
        2,
        100,
        SPACE_SM,
        Some([0, 1]),
    )
    .unwrap();
    assert!(rows.last().unwrap().page > 0);
    assert_eq!(
        rows.iter().map(|row| row.text.as_str()).collect::<String>(),
        expected
    );
    assert_disjoint(&rows);
    for row in &rows {
        assert_eq!(row.bounds.width, 1280 - 24 - EDGE * 2);
        assert_eq!(row.bounds.x, (12 + EDGE) as i16);
    }
}

#[test]
fn naming_field_moves_as_one_cluster_to_full_width_continuation() {
    let mut inputs = (0..18)
        .map(|index| item(&format!("Previous choice {index}"), Some(0)))
        .collect::<Vec<_>>();
    let action = FaceControl {
        action: 3,
        argument: None,
    };
    let argument = FaceControl {
        action: 3,
        argument: Some(0),
    };
    let mut field = vec![
        item("Choose a tradition for the next name suggestion.", Some(1)),
        item("Use naming tradition", Some(1)),
        item("Naming tradition: welsh · Left/Right choose", Some(1)),
    ];
    for item in &mut field {
        item.subject_index = Some(7);
    }
    field[1].control = Some(action);
    field[2].control = Some(argument);
    inputs.extend(field);
    let rows = admit_configured(
        inputs,
        screen(1280, 1200),
        12,
        2,
        100,
        SPACE_SM,
        Some([0, 1]),
    )
    .unwrap();
    assert_disjoint(&rows);
    assert!(rows[..18].iter().all(|row| row.page == 0));
    assert_eq!(rows.len(), 21);
    assert!(rows[18..].iter().all(|row| row.page == 1));
    assert!(
        rows[18..]
            .iter()
            .all(|row| row.bounds.width == 1280 - 24 - EDGE * 2)
    );
    assert_eq!(rows[19].control, Some(action));
    assert_eq!(rows[20].control, Some(argument));
    assert_eq!(rows[18].panel_bounds.unwrap().width, 1280 - 24);
}
