//! CPU structure/meaning/geometry conformance; no scanout acknowledgement.
use super::*;
use alloc::{format, vec};
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn child(node: u8) -> ComponentLayoutChild {
    ComponentLayoutChild::new(1, 0, 1, 0, node).unwrap()
}
fn group(nodes: &[u8], keep: bool) -> ComponentLayoutGroup {
    ComponentLayoutGroup::new(
        Alignment::Start,
        if keep {
            Break::Keep
        } else {
            Break::AccessibleContinuation
        },
        BoundedSequence::try_from_iter(nodes.iter().map(|n| child(*n))).unwrap(),
        8,
        Alignment::Stretch,
        1,
        1,
        8,
    )
    .unwrap()
}
fn node(group: ComponentLayoutGroup, row: bool) -> ComponentLayoutNode {
    let make = if row {
        ComponentLayoutNode::row
    } else {
        ComponentLayoutNode::column
    };
    make(
        *group.block_alignment(),
        *group.break_policy(),
        group.children().clone(),
        *group.gap_em_sixteenths(),
        *group.inline_alignment(),
        *group.inline_grow(),
        *group.inline_shrink(),
        *group.padding_em_sixteenths(),
    )
    .unwrap()
}
fn candidate(nodes: Vec<ComponentLayoutNode>) -> ComponentLayoutCandidate {
    ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0).unwrap()
}
pub(super) fn wide(keep: bool) -> ComponentLayoutCandidate {
    candidate(vec![
        node(group(&[1, 4], false), false),
        node(group(&[2, 3], keep), true),
        ComponentLayoutNode::region(0).unwrap(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::Fallback,
    ])
}
pub(super) fn narrow() -> ComponentLayoutCandidate {
    candidate(vec![
        node(group(&[1, 2, 3], false), false),
        ComponentLayoutNode::region(0).unwrap(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::Fallback,
    ])
}
pub(super) fn prepared<'a>(
    face: &'a Face,
    candidates: Vec<ComponentLayoutCandidate>,
) -> PreparedComponentLayout<'a> {
    let recipe = ComponentRecipeInfo::new(
        0,
        "test/mechanical@1".into(),
        1,
        280,
        BoundedSequence::try_from_iter([("z-first", 0), ("a-field", 1)].map(|(subject, group)| {
            ComponentRegionInfo::new(
                group,
                subject.into(),
                20,
                ComponentSubjectSelector::subject(subject.into()).unwrap(),
            )
            .unwrap()
        }))
        .unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let structure = ComponentLayoutInfo::new(
        BoundedSequence::try_from_iter(candidates).unwrap(),
        "test/mechanical-layout@1".into(),
        "test/mechanical@1".into(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    prepare_component_layout(structure, recipe, face).unwrap()
}
pub(super) fn item(text: &str, region: Option<usize>, control: bool) -> document::Item {
    document::Item {
        text: text.into(),
        role: GraphicsTextRole::Label,
        paint: GraphicsPaintRole::Foreground,
        control: control.then_some(FaceControl {
            action: 1,
            argument: None,
        }),
        indent: 0,
        subject_index: region,
        region: region.map(|r| r as u8),
        region_order: region,
        checkbox: None,
        icon: None,
    }
}
fn screen(width: u16, height: u16) -> LayoutRect {
    LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
    }
}
pub(super) fn run(
    items: Vec<document::Item>,
    width: u16,
    height: u16,
    profile: &PreparedComponentLayout<'_>,
) -> Result<Vec<Row>, FaceSceneError> {
    admit(
        items,
        screen(width, height),
        profile,
        100,
        SPACE_SM,
        Typography::EDITORIAL,
    )
}
fn intersects(a: LayoutRect, b: LayoutRect) -> bool {
    i32::from(a.x) < i32::from(b.x) + i32::from(b.width)
        && i32::from(b.x) < i32::from(a.x) + i32::from(a.width)
        && i32::from(a.y) < i32::from(b.y) + i32::from(b.height)
        && i32::from(b.y) < i32::from(a.y) + i32::from(a.height)
}
pub(super) fn assert_geometry(rows: &[Row], width: u16, height: u16) {
    let pages = rows.iter().map(|r| r.page).max().unwrap_or(0) + 1;
    for page in 0..pages {
        let same: Vec<_> = rows.iter().filter(|r| r.page == page).collect();
        assert!(same.len() <= MAX_PAGE_ROWS);
        for (i, row) in same.iter().enumerate() {
            assert!(row.bounds.x >= 0 && row.bounds.y >= 0);
            assert!(i32::from(row.bounds.x) + i32::from(row.bounds.width) <= i32::from(width));
            assert!(i32::from(row.bounds.y) + i32::from(row.bounds.height) <= i32::from(height));
            for other in &same[i + 1..] {
                assert!(!intersects(row.bounds, other.bounds));
            }
        }
    }
}
#[test]
fn nested_rows_columns_preserve_all_content_and_original_controls() {
    let face = crate::native_face_scene::tests::face(7);
    let prepared = prepared(&face, vec![wide(false), narrow()]);
    let items = || {
        vec![
            item("Second region encountered first", Some(1), true),
            item("First region", Some(0), false),
            item("Unknown semantic material", None, false),
        ]
    };
    let first = run(items(), 1000, 700, &prepared).unwrap();
    let second = run(items(), 1000, 700, &prepared).unwrap();
    assert_eq!(
        first
            .iter()
            .map(|r| (&r.text, r.bounds, r.page, r.control))
            .collect::<Vec<_>>(),
        second
            .iter()
            .map(|r| (&r.text, r.bounds, r.page, r.control))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        first.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        [
            "Second region encountered first",
            "First region",
            "Unknown semantic material"
        ]
    );
    assert_eq!(
        first[0].control,
        Some(FaceControl {
            action: 1,
            argument: None
        })
    );
    assert_geometry(&first, 1000, 700);
    assert_eq!(face.revision, prepared.face().revision);
}
#[test]
fn measured_candidate_choice_uses_wrapped_demand_and_minimum_conflicts_refuse() {
    let face = crate::native_face_scene::tests::face(7);
    let prepared = prepared(&face, vec![wide(false), narrow()]);
    let rows = run(
        vec![
            item(
                "A long label requiring a measured readable horizontal extent",
                Some(0),
                false,
            ),
            item(
                "Another long label requiring a measured readable horizontal extent",
                Some(1),
                false,
            ),
        ],
        320,
        900,
        &prepared,
    )
    .unwrap();
    assert_geometry(&rows, 320, 900);
    // Narrow alternative removes the nested row's extra inset and avoids its
    // extra wrapping, rather than consulting a fixed viewport threshold.
    assert_eq!(rows[0].bounds.x, rows[1].bounds.x);
    assert_eq!(
        run(vec![item("W", Some(0), true)], 20, 300, &prepared).err(),
        Some(FaceSceneError::InvalidExtent)
    );
}
#[test]
fn resource_pressure_paginates_every_row_without_overlap_or_binding_loss() {
    let face = crate::native_face_scene::tests::face(7);
    let prepared = prepared(&face, vec![wide(false), narrow()]);
    let items = (0..65)
        .map(|i| item(&format!("Original control {i}"), Some(i % 2), true))
        .collect();
    let rows = run(items, 1100, 1400, &prepared).unwrap();
    assert_eq!(rows.len(), 65);
    assert_geometry(&rows, 1100, 1400);
    assert!(rows.iter().any(|r| r.page > 0));
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.text, format!("Original control {i}"));
        assert!(row.control.is_some());
    }
}
#[test]
fn impossible_keep_and_unsupported_grid_are_explicit() {
    let face = crate::native_face_scene::tests::face(7);
    let keep = prepared(&face, vec![wide(true)]);
    let many = || {
        (0..40)
            .map(|i| item(&format!("Control {i}"), Some(0), true))
            .collect()
    };
    assert!(run(many(), 800, 350, &keep).is_err());
    let continuing = prepared(&face, vec![wide(true), narrow()]);
    assert_eq!(run(many(), 800, 350, &continuing).unwrap().len(), 40);
    let grid = ComponentLayoutNode::grid(
        3,
        ComponentLayoutGroup::new(
            Alignment::Start,
            Break::AccessibleContinuation,
            BoundedSequence::try_from_iter(
                (1..=3).map(|n| ComponentLayoutChild::new(1, n - 1, 1, 0, n).unwrap()),
            )
            .unwrap(),
            0,
            Alignment::Stretch,
            1,
            1,
            0,
        )
        .unwrap(),
        1,
    )
    .unwrap();
    let unsupported = prepared(
        &face,
        vec![candidate(vec![
            grid,
            ComponentLayoutNode::region(0).unwrap(),
            ComponentLayoutNode::region(1).unwrap(),
            ComponentLayoutNode::Fallback,
        ])],
    );
    assert_eq!(
        run(vec![], 800, 600, &unsupported).err(),
        Some(FaceSceneError::UnsupportedStructure)
    );
}
#[test]
fn real_face_scene_uses_existing_frame_hits_and_resource_admission() {
    let face = crate::native_face_scene::tests::face(7);
    let prepared = prepared(&face, vec![wide(false), narrow()]);
    let expression = admit_visual_expression(
        Ok(VisualExpression::phosphor()),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    let mut scene =
        NativeFaceScene::prepare_structured(&prepared, 1000, 750, true, &expression).unwrap();
    let mut hits = 0;
    for page in 0..scene.pages() {
        scene.page = page;
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
        for hit in &frame.hits {
            let x = hit.bounds.x + (hit.bounds.width / 2) as i16;
            let y = hit.bounds.y + (hit.bounds.height / 2) as i16;
            assert_eq!(frame.hit_test(x, y).unwrap().control, hit.control);
            assert_eq!(hit.action, &face.actions[hit.control.action]);
        }
        hits += frame.hits.len();
    }
    assert!(hits > 0);
    let expected: alloc::collections::BTreeSet<_> = document::prepare(&face, true)
        .unwrap()
        .0
        .iter()
        .filter_map(|item| item.control.map(|c| (c.action, c.argument)))
        .collect();
    let actual: alloc::collections::BTreeSet<_> = scene
        .primary
        .iter()
        .filter_map(|row| row.control.map(|c| (c.action, c.argument)))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(&scene.face, &face);
    assert_geometry(&scene.primary, 1000, 750);
}

#[test]
fn unicode_chunks_large_text_and_selected_scale_preserve_complete_payload() {
    let face = crate::native_face_scene::tests::face(7);
    let profile = prepared(&face, vec![wide(false), narrow()]);
    let original = "é W e\u{301} translated meaning ".repeat(35);
    let rows = admit(
        vec![item(&original, Some(0), true)],
        screen(350, 420),
        &profile,
        200,
        SPACE_SM,
        Typography::EDITORIAL,
    )
    .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.text.as_str())
            .collect::<alloc::string::String>(),
        original
    );
    assert!(
        rows.iter()
            .all(|row| row.text.len() <= MAX_GRAPHICS_TEXT_BYTES)
    );
    assert_geometry(&rows, 350, 420);
}

#[test]
fn allocation_weights_alignment_and_preferred_sum_have_finite_native_bounds() {
    let items = vec![item("W", Some(0), false), item("W", Some(1), false)];
    let context = Context {
        items: &items,
        width: 60000,
        scale: 200,
        padding: SPACE_SM,
        typography: Typography::EDITORIAL,
        rows: core::cell::Cell::new(0),
    };
    let mut demands = [Demand::default(); 32];
    demands[0] = Demand {
        minimum: 10,
        preferred: 20,
        inline_grow: 1,
    };
    demands[1] = Demand {
        minimum: 10,
        preferred: 20,
        inline_grow: 3,
    };
    let row = node(super::tests::group(&[0, 1], false), true);
    let Node::Group(group) = structure::node(&context, &row).unwrap() else {
        panic!()
    };
    let widths = structure::widths(&group, 200, &demands).unwrap();
    assert!(widths[1] > widths[0]);
    assert_eq!(widths[0] + widths[1] + group.gap, 200);
    assert!(structure::widths(&group, 10, &demands).is_err());
    let mut nodes = vec![node(
        super::tests::group(&(1..=9).collect::<Vec<_>>(), false),
        true,
    )];
    nodes.extend((0..8).map(|region| ComponentLayoutNode::region(region).unwrap()));
    nodes.push(ComponentLayoutNode::Fallback);
    let candidate = candidate(nodes);
    let items: Vec<_> = (0..9)
        .map(|region| {
            let mut value = item(
                &"W".repeat(192),
                if region == 8 { None } else { Some(region) },
                false,
            );
            value.role = GraphicsTextRole::Title;
            value
        })
        .collect();
    let context = Context {
        items: &items,
        width: 8000,
        scale: 200,
        padding: SPACE_SM,
        typography: Typography::EDITORIAL,
        rows: core::cell::Cell::new(0),
    };
    let mut demands = [Demand::default(); 32];
    let demand = structure::measure(&context, &candidate, 0, &mut demands).unwrap();
    assert!(
        demands[1..=9]
            .iter()
            .map(|d| u32::from(d.preferred))
            .sum::<u32>()
            > u32::from(u16::MAX)
    );
    assert!(demand.minimum < 8000);
    assert_eq!(demand.preferred, 8000);
}
#[test]
fn retained_row_capacity_is_refused_before_the_next_entry() {
    let items = vec![item("one", Some(0), false), item("two", Some(0), false)];
    let context = Context {
        items: &items,
        width: 600,
        scale: 100,
        padding: SPACE_SM,
        typography: Typography::EDITORIAL,
        rows: core::cell::Cell::new(MAX_ROWS - 1),
    };
    assert_eq!(
        leaves::arrange(&context, Some(0), 600, 500).err(),
        Some(FaceSceneError::DocumentBound)
    );
    assert_eq!(context.rows.get(), MAX_ROWS);
}
