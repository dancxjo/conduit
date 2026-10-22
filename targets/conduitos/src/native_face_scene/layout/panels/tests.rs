//! CPU layout/graphics conformance only; no owner or guest input enactment.
use super::*;
use crate::native_face_scene::{NativeFaceScene, tests as scene_tests};
use alloc::{format, vec};

pub(super) fn screen(width: u16, height: u16) -> LayoutRect {
    LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
    }
}
pub(super) fn item(text: &str, region: Option<u8>) -> document::Item {
    document::Item {
        text: text.into(),
        role: GraphicsTextRole::Label,
        paint: GraphicsPaintRole::Foreground,
        control: None,
        indent: 0,
        subject_index: None,
        region,
        region_order: region.map(usize::from),
        checkbox: None,
        icon: None,
    }
}
fn end(row: &Row) -> i32 {
    i32::from(row.bounds.y) + i32::from(row.bounds.height)
}
fn intersects(a: LayoutRect, b: LayoutRect) -> bool {
    i32::from(a.x) < i32::from(b.x) + i32::from(b.width)
        && i32::from(b.x) < i32::from(a.x) + i32::from(a.width)
        && i32::from(a.y) < i32::from(b.y) + i32::from(b.height)
        && i32::from(b.y) < i32::from(a.y) + i32::from(a.height)
}
pub(super) fn assert_disjoint(rows: &[Row]) {
    for (index, row) in rows.iter().enumerate() {
        for other in &rows[index + 1..] {
            if row.page == other.page {
                assert!(!intersects(row.bounds, other.bounds));
            }
        }
    }
}

#[test]
fn empty_group_does_not_add_panel_geometry_or_displace_reading() {
    let prepare = |items| {
        admit_configured(items, screen(1280, 800), 12, 2, 100, SPACE_SM, Some([0, 1])).unwrap()
    };
    let plain = prepare(vec![item("Reading", None)]);
    let empty = prepare(vec![item("", Some(0)), item("Reading", None)]);
    assert_eq!(plain[0].bounds, empty[0].bounds);
    assert!(empty[0].panel_bounds.is_none());
}

#[test]
fn authored_pair_is_independent_of_early_review_reading_order() {
    let groups = authored_panel_groups([9, 9, 4, 4, 23].into_iter());
    assert_eq!(groups, Some([9, 4]));
    let rows = admit_configured(
        vec![
            item("Review first", Some(23)),
            item("Title", None),
            item("Left one", Some(9)),
            item("Left two", Some(9)),
            item("Right one", Some(4)),
            item("Review last", Some(23)),
            item("Finish", None),
        ],
        screen(1280, 800),
        12,
        2,
        100,
        SPACE_SM,
        groups,
    )
    .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(),
        [
            "Review first",
            "Title",
            "Left one",
            "Left two",
            "Right one",
            "Review last",
            "Finish"
        ]
    );
    assert_eq!(
        rows[2].bounds.y, rows[4].bounds.y,
        "groups begin alongside each other before overflow"
    );
    assert!(rows[4].bounds.x > rows[2].bounds.x);
    assert!(rows[0].bounds.width > rows[2].bounds.width);
    assert!(rows[5].bounds.width > rows[4].bounds.width);
    assert!(i32::from(rows[5].bounds.y) > end(&rows[3]).max(end(&rows[4])));
    assert_eq!(
        rows.iter().filter(|row| row.panel_bounds.is_some()).count(),
        2
    );
    assert_disjoint(&rows);
}

#[test]
fn interleaved_unmatched_reading_uses_complete_tail_without_losing_rows() {
    let rows = admit_configured(
        vec![
            item("Left", Some(8)),
            item("Complete unmatched fact", None),
            item("Right", Some(2)),
            item("Later left", Some(8)),
            item("Review", Some(7)),
        ],
        screen(1280, 800),
        12,
        2,
        100,
        SPACE_SM,
        Some([8, 2]),
    )
    .unwrap();
    assert_eq!(rows.len(), 5);
    assert_disjoint(&rows);
    assert!(rows[1].bounds.width > rows[0].bounds.width);
    assert!(i32::from(rows[1].bounds.y) > end(&rows[0]));
    assert!(i32::from(rows[4].bounds.y) > end(&rows[2]).max(end(&rows[3])));
}

#[test]
fn narrow_configuration_preserves_existing_linear_reading_geometry() {
    let inputs = || {
        vec![
            item("Earlier review", Some(2)),
            item("Left", Some(0)),
            item("Right", Some(1)),
        ]
    };
    for width in [320, 640] {
        let rows = admit_configured(
            inputs(),
            screen(width, 480),
            12,
            1,
            100,
            SPACE_SM,
            Some([0, 1]),
        )
        .unwrap();
        let baseline =
            admit_with_expression(inputs(), screen(width, 480), 12, 1, 100, SPACE_SM).unwrap();
        assert_eq!(rows.len(), baseline.len());
        for (row, old) in rows.iter().zip(baseline) {
            assert_eq!(
                (row.text.as_str(), row.bounds, row.page),
                (old.text.as_str(), old.bounds, old.page)
            );
            assert!(row.panel_bounds.is_none());
        }
    }
}

#[test]
fn measured_unicode_command_pressure_retains_every_byte_and_reserves_panel_outlines() {
    let text = "Éléonore 雪 · ".repeat(60);
    let mut inputs = vec![item("Heading", None)];
    for group in [6, 3] {
        for _ in 0..18 {
            let mut row = item(&text, Some(group));
            row.checkbox = Some(false);
            inputs.push(row);
        }
    }
    inputs.push(item("Full width completion", Some(5)));
    let rows = admit_configured(
        inputs,
        screen(1280, 800),
        12,
        2,
        200,
        SPACE_SM,
        Some([6, 3]),
    )
    .unwrap();
    assert_disjoint(&rows);
    let restored: String = rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(
        restored,
        format!("Heading{}Full width completion", text.repeat(36))
    );
    let pages = rows.iter().map(|row| row.page + 1).max().unwrap();
    assert!(pages > 1);
    for page in 0..pages {
        let mut scene = GraphicsScene::empty();
        // Mirror the complete frame's admitted fixed chrome + one focus marker.
        for _ in 0..chrome::COMMANDS + 1 {
            push(
                &mut scene,
                GraphicsCommand::rect(
                    LayoutRect {
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                    },
                    screen(1280, 800),
                    GraphicsPaintRole::Muted,
                    GraphicsShapeStyle::Fill,
                ),
            )
            .unwrap();
        }
        for row in rows.iter().filter(|row| row.page == page) {
            append_panel_chrome(&mut scene, row, screen(1280, 800)).unwrap();
        }
        for row in rows.iter().filter(|row| row.page == page) {
            append_row_chrome(&mut scene, row, screen(1280, 800), row.paint, false, false).unwrap();
            crate::native_face_scene::boolean_choice::append(&mut scene, row, screen(1280, 800))
                .unwrap();
            push(
                &mut scene,
                GraphicsCommand::text(row.text_bounds(), screen(1280, 800), row.paint, &row.text)
                    .and_then(|command| command.with_text_role(row.role)),
            )
            .unwrap();
        }
        assert!(scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
        assert!(
            scene.commands().iter().all(
                |command| command.clip_class() == conduit_face::GraphicsClipClass::FullyVisible
            )
        );
    }
}

#[test]
fn layout_preserves_the_original_face_control_traversal_and_complete_details() {
    let face = scene_tests::face(7);
    let original = face.clone();
    let mut scene = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
    let old_details: String = scene.details.iter().map(|row| row.text.as_str()).collect();
    let (mut items, _) = document::prepare(&face, true).unwrap();
    for row in &mut items {
        row.region = row
            .subject_index
            .map(|index| if index == 0 { 11 } else { 3 });
    }
    let expected: Vec<_> = scene.primary.iter().filter_map(|row| row.control).collect();
    scene.primary =
        admit_configured(items, scene.screen, 12, 2, 100, SPACE_SM, Some([11, 3])).unwrap();
    assert_eq!(
        scene
            .primary
            .iter()
            .filter_map(|row| row.control)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(scene.face(), &original);
    assert_eq!(
        scene
            .details
            .iter()
            .map(|row| row.text.as_str())
            .collect::<String>(),
        old_details
    );
    for control in expected {
        assert_eq!(scene.focus_next(true), Some(control));
        let frame = scene.frame().unwrap();
        let hit = frame
            .hits
            .iter()
            .find(|hit| hit.control == control)
            .unwrap();
        assert_eq!(
            frame
                .hit_test(hit.bounds.x + 1, hit.bounds.y + 1)
                .unwrap()
                .control,
            control
        );
        assert_eq!(
            scene
                .resolve(frame.face_id, frame.revision, control)
                .unwrap()
                .0,
            hit.action
        );
    }
}

#[test]
fn impossible_panel_width_refuses_without_an_overflow_scene() {
    assert!(matches!(
        admit_configured(
            vec![item("Named action", Some(0))],
            screen(640, 480),
            220,
            2,
            100,
            SPACE_SM,
            Some([0, 1])
        ),
        Err(FaceSceneError::InvalidExtent)
    ));
    assert_eq!(authored_panel_groups([4, 4, 4].into_iter()), None);
}

#[test]
fn panel_metadata_has_a_finite_preparation_storage_delta() {
    #[allow(dead_code)]
    struct BeforePanelMetadata {
        text: String,
        role: GraphicsTextRole,
        paint: GraphicsPaintRole,
        control: Option<FaceControl>,
        subject_index: Option<usize>,
        page: usize,
        bounds: LayoutRect,
        region_start: bool,
        padding: u16,
        checkbox: Option<bool>,
    }
    let permutation = core::mem::size_of::<(usize, usize, usize)>();
    let indexed_item = core::mem::size_of::<(usize, document::Item)>();
    std::println!(
        "indexed_item={indexed_item} permutation_row={permutation} maximum_permutation={} conservative_preparation_storage={}",
        permutation * MAX_ROWS,
        (core::mem::size_of::<Row>() + indexed_item + permutation) * MAX_ROWS
    );
    let before = core::mem::size_of::<BeforePanelMetadata>();
    let after = core::mem::size_of::<Row>();
    let maximum_delta = core::mem::size_of::<Option<LayoutRect>>() + core::mem::align_of::<Row>();
    assert!(after <= before + maximum_delta);
    std::println!(
        "row_before={before} row_after={after} max_rows={MAX_ROWS} maximum_row_storage={} metadata_delta={}",
        after * MAX_ROWS,
        after.saturating_sub(before) * MAX_ROWS
    );
}

#[path = "tests/creche.rs"]
mod creche;

#[test]
fn nonmonotonic_pages_include_later_visual_pages_and_preserve_turning() {
    let mut items: Vec<_> = (0..30).map(|_| item("Right", Some(1))).collect();
    items.push(item("Left read last", Some(0)));
    let rows =
        admit_configured(items, screen(1280, 800), 12, 2, 100, SPACE_SM, Some([0, 1])).unwrap();
    assert_eq!(rows.last().unwrap().page, 0);
    let maximum = rows.iter().map(|row| row.page).max().unwrap();
    assert!(maximum > 0);
    let mut scene = NativeFaceScene::prepare(scene_tests::face(7), 1280, 800).unwrap();
    scene.primary = rows;
    assert_eq!(scene.pages(), maximum + 1);
    for page in 1..=maximum {
        scene.turn_page(true);
        assert_eq!(scene.page(), page);
    }
    scene.turn_page(true);
    assert_eq!(scene.page(), maximum);
}

#[test]
fn chrome_valid_tiny_panel_extent_refuses_without_arithmetic_panic() {
    assert!(matches!(
        admit_configured(
            vec![item("Visible", Some(0))],
            screen(640, 127),
            12,
            2,
            100,
            SPACE_SM,
            Some([0, 1])
        ),
        Err(FaceSceneError::InvalidExtent)
    ));
}
