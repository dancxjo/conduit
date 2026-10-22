//! CPU pagination conformance, not owner/guest interaction proof.
use super::*;

fn item(text: &str, subject: Option<usize>, control: Option<FaceControl>) -> document::Item {
    document::Item {
        text: text.into(),
        role: GraphicsTextRole::Label,
        paint: GraphicsPaintRole::Foreground,
        control,
        indent: 0,
        subject_index: subject,
        region: None,
        region_order: None,
        checkbox: None,
        icon: None,
    }
}

#[test]
fn field_help_action_and_argument_stay_together_at_row_capacity() {
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width: 1280,
        height: 1024,
    };
    let mut items = (0..18)
        .map(|_| item("Prefix", None, None))
        .collect::<Vec<_>>();
    items.extend([
        item("Choose a naming tradition", Some(7), None),
        item(
            "Change tradition",
            Some(7),
            Some(FaceControl {
                action: 3,
                argument: None,
            }),
        ),
        item(
            "Current tradition: roman",
            Some(7),
            Some(FaceControl {
                action: 3,
                argument: Some(0),
            }),
        ),
    ]);
    // Isolate the 20-row cap: all first twenty rows fit height and commands.
    let chrome = chrome::Chrome::prepare(screen, 100).unwrap();
    let height = measure_prefix(
        "Prefix",
        GraphicsTextRole::Label,
        1200,
        900,
        100,
        Typography::EDITORIAL,
    )
    .unwrap()
    .1;
    assert!(chrome.top + 20 * (height + SPACE_SM * 3) <= chrome.bottom);
    let rows = admit_with_expression(items, screen, 12, 1, 100, SPACE_SM).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .len(),
        21
    );
    assert_eq!(rows[18].page, rows[19].page);
    assert_eq!(
        rows[19].page, rows[20].page,
        "argument orphaned from exact same-subject action/help"
    );
    assert_eq!(
        rows[20].control,
        Some(FaceControl {
            action: 3,
            argument: Some(0)
        })
    );
}

fn field() -> [document::Item; 3] {
    [
        item("Help", Some(7), None),
        item(
            "Act",
            Some(7),
            Some(FaceControl {
                action: 3,
                argument: None,
            }),
        ),
        item(
            "Value",
            Some(7),
            Some(FaceControl {
                action: 3,
                argument: Some(0),
            }),
        ),
    ]
}

fn check_field(rows: &[Row], start: usize) {
    assert!(
        rows[start..start + 3]
            .iter()
            .all(|row| row.page == rows[start].page)
    );
    assert!(
        rows[start..start + 3]
            .iter()
            .all(|row| row.bounds.x == rows[start].bounds.x)
    );
    assert_eq!(
        rows[start + 1].control,
        Some(FaceControl {
            action: 3,
            argument: None
        })
    );
    assert_eq!(
        rows[start + 2].control,
        Some(FaceControl {
            action: 3,
            argument: Some(0)
        })
    );
    assert_eq!(
        rows[start..start + 3]
            .iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>(),
        ["Help", "Act", "Value"]
    );
}

#[test]
fn large_text_height_pressure_keeps_field_in_flat_and_panel_geometry() {
    for panels in [false, true] {
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: if panels { 1280 } else { 640 },
            height: 480,
        };
        let chrome = chrome::Chrome::prepare(screen, 200).unwrap();
        let edge = if panels { 8 } else { 0 };
        let available = chrome.bottom - chrome.top - edge * 2;
        let height = measure_prefix(
            "Prefix",
            GraphicsTextRole::Label,
            500,
            available - SPACE_SM * 2,
            200,
            Typography::EDITORIAL,
        )
        .unwrap()
        .1 + SPACE_SM * 2;
        let count = usize::from((available + SPACE_SM) / (height + SPACE_SM)) - 2;
        assert!(count > 0 && 3 * height + 2 * SPACE_SM <= available);
        let mut items = (0..count)
            .map(|_| item("Prefix", None, None))
            .collect::<Vec<_>>();
        items.extend(field());
        if panels {
            for item in &mut items {
                item.region = Some(0);
                item.region_order = Some(0);
            }
        }
        let rows = admit_configured(
            items,
            screen,
            12,
            if panels { 2 } else { 1 },
            200,
            SPACE_SM,
            Some([0, 1]),
        )
        .unwrap();
        check_field(&rows, count);
        assert!(
            rows[count].page > rows[count - 1].page,
            "height pressure must move the complete cluster"
        );
    }
}

#[test]
fn graphics_command_pressure_moves_whole_field_before_global_limit() {
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width: 1280,
        height: 1024,
    };
    let mut items = (0..13)
        .map(|_| {
            let mut row = item("Prefix", None, None);
            row.checkbox = Some(false);
            row
        })
        .collect::<Vec<_>>();
    items.extend(field());
    const {
        assert!(13 * 3 + chrome::COMMANDS + 1 < conduit_face::MAX_GRAPHICS_COMMANDS);
        assert!(13 * 3 + chrome::COMMANDS + 1 + 5 > conduit_face::MAX_GRAPHICS_COMMANDS);
    }
    let rows = admit_with_expression(items, screen, 12, 1, 100, SPACE_SM).unwrap();
    check_field(&rows, 13);
    assert_eq!(rows[12].page, 0);
    assert_eq!(rows[13].page, 1);
}

#[test]
fn oversized_help_continues_without_empty_pages_lost_text_or_duplicate_controls() {
    for panels in [false, true] {
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        };
        let help = "Long Unicode help é界 ".repeat(120);
        let mut items = field().into_iter().collect::<Vec<_>>();
        items[0].text = help.clone();
        if panels {
            for item in &mut items {
                item.region = Some(0);
                item.region_order = Some(0);
            }
        }
        let rows = admit_configured(
            items,
            screen,
            12,
            if panels { 2 } else { 1 },
            200,
            SPACE_SM,
            Some([0, 1]),
        )
        .unwrap();
        assert_eq!(
            rows[0].page, 0,
            "oversized cluster must not advance an empty page"
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r.control.is_none())
                .map(|r| r.text.as_str())
                .collect::<String>(),
            help
        );
        assert_eq!(
            rows.iter().filter_map(|r| r.control).collect::<Vec<_>>(),
            [
                FaceControl {
                    action: 3,
                    argument: None
                },
                FaceControl {
                    action: 3,
                    argument: Some(0)
                }
            ]
        );
        let last = rows.last().unwrap().page;
        assert!(last > 0 && last < 32);
        for page in 0..=last {
            let page_rows = rows.iter().filter(|r| r.page == page).collect::<Vec<_>>();
            assert!(!page_rows.is_empty() && page_rows.len() <= MAX_PAGE_ROWS);
            assert!(
                page_rows
                    .iter()
                    .all(|r| r.text.len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES)
            );
        }
    }
}

#[test]
fn actual_face_target_rows_keep_typed_refs_and_exact_reading_without_mutation() {
    let face = crate::native_face_scene::tests::face(7);
    let before = face.clone();
    let (primary, _) = document::prepare(&face, true).unwrap();
    let target = face
        .subjects
        .iter()
        .position(|s| s.identity == face.actions[0].target)
        .unwrap();
    let associated = primary
        .into_iter()
        .filter(|i| i.subject_index == Some(target))
        .collect::<Vec<_>>();
    assert!(
        associated
            .iter()
            .any(|i| i.control.is_some_and(|c| c.argument.is_some()))
    );
    let expected_text = associated
        .iter()
        .map(|i| i.text.clone())
        .collect::<Vec<_>>();
    let expected_controls = associated
        .iter()
        .filter_map(|i| i.control)
        .collect::<Vec<_>>();
    let count = MAX_PAGE_ROWS - 1;
    let mut items = (0..count)
        .map(|_| item("Prefix", None, None))
        .collect::<Vec<_>>();
    items.extend(associated);
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width: 1280,
        height: 1024,
    };
    let rows = admit_with_expression(items, screen, 12, 1, 100, SPACE_SM).unwrap();
    assert!(rows[count..].iter().all(|r| r.page == rows[count].page));
    assert_eq!(
        rows[count..]
            .iter()
            .map(|r| r.text.clone())
            .collect::<Vec<_>>(),
        expected_text
    );
    assert_eq!(
        rows[count..]
            .iter()
            .filter_map(|r| r.control)
            .collect::<Vec<_>>(),
        expected_controls
    );
    assert_eq!(face, before);
}

#[test]
fn panel_preflight_respects_global_row_and_command_caps_not_only_lane_height() {
    for command_pressure in [false, true] {
        let count = if command_pressure { 12 } else { 18 };
        let mut items = (0..count)
            .map(|_| {
                let mut row = item("Prefix", None, None);
                row.checkbox = command_pressure.then_some(false);
                row
            })
            .collect::<Vec<_>>();
        items.extend(field());
        for item in &mut items {
            item.region = Some(0);
            item.region_order = Some(0);
        }
        let rows = admit_configured(
            items,
            LayoutRect {
                x: 0,
                y: 0,
                width: 1280,
                height: 1024,
            },
            12,
            2,
            100,
            SPACE_SM,
            Some([0, 1]),
        )
        .unwrap();
        check_field(&rows, count);
        assert_eq!(rows[count - 1].page, 0);
        assert_eq!(rows[count].page, 1);
    }
}
