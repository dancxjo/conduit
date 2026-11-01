//! Actual selected text/Face form realization, not hand-authored rectangles.
use super::tests::{assert_geometry, item};
use super::*;
use alloc::{collections::BTreeSet, format, vec};
use conduit_face::*;
use conduit_plot::rust_binding::BoundedSequence;

#[test]
fn independent_bound_form_has_shared_tracks_spans_and_unchanged_semantic_traversal() {
    let face = super::grid_fixture::face();
    let profile = super::grid_fixture::profile(&face);
    let (original, _) = document::prepare(&face, true).unwrap();
    let expected = original
        .iter()
        .map(|item| item.text.as_str())
        .collect::<String>();
    let expected_controls: BTreeSet<_> = original
        .iter()
        .filter_map(|item| item.control.map(|c| (c.action, c.argument)))
        .collect();
    let prepare = || {
        NativeFaceScene::prepare_structured(
            &profile,
            1100,
            900,
            true,
            &super::grid_fixture::expression(100),
        )
        .unwrap()
    };
    let mut scene = prepare();
    let repeat = prepare();
    assert_eq!(
        scene
            .primary
            .iter()
            .map(|row| (&row.text, row.bounds, row.page, row.control))
            .collect::<Vec<_>>(),
        repeat
            .primary
            .iter()
            .map(|row| (&row.text, row.bounds, row.page, row.control))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        scene
            .primary
            .iter()
            .map(|row| row.text.as_str())
            .collect::<String>(),
        expected
    );
    assert_eq!(
        scene
            .primary
            .iter()
            .filter_map(|row| row.control.map(|c| (c.action, c.argument)))
            .collect::<BTreeSet<_>>(),
        expected_controls
    );
    let row = |identity: &str| {
        scene
            .primary
            .iter()
            .find(|row| {
                row.subject_index
                    .is_some_and(|i| face.subjects[i].identity == identity)
            })
            .unwrap()
    };
    assert_eq!(row("name-label").bounds.x, row("email-label").bounds.x);
    assert_eq!(row("name").bounds.x, row("email").bounds.x);
    assert!(row("email").bounds.x > row("email-label").bounds.x);
    assert!(row("name-help").bounds.width > row("name").bounds.width);
    assert_geometry(&scene.primary, 1100, 900);
    let mut hits = BTreeSet::new();
    for page in 0..scene.pages() {
        scene.page = page;
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        for hit in &frame.hits {
            assert!(face.actions[hit.control.action].availability.is_available());
            assert_eq!(
                frame
                    .hit_test(
                        hit.bounds.x + (hit.bounds.width / 2) as i16,
                        hit.bounds.y + (hit.bounds.height / 2) as i16
                    )
                    .unwrap()
                    .control,
                hit.control
            );
            hits.insert(hit.control.action);
        }
    }
    assert!(hits.contains(&0));
    assert!(!hits.contains(&1));
    assert_eq!(scene.face(), &face);
}
#[test]
fn track_contributions_support_zero_single_and_spanning_demands_without_empty_minima() {
    let items = vec![];
    let context = Context {
        items: &items,
        width: 1000,
        scale: 100,
        padding: SPACE_SM,
        gap: SPACE_SM,
        typography: Typography::EDITORIAL,
        rows: core::cell::Cell::new(0),
    };
    let group = |children| {
        ComponentLayoutGroup::new(
            Alignment::Start,
            Break::AccessibleContinuation,
            BoundedSequence::try_from_iter(children).unwrap(),
            0,
            Alignment::Stretch,
            1,
            1,
            0,
            0,
        )
        .unwrap()
    };
    let node = ComponentLayoutNode::grid(1, group(vec![]), 4).unwrap();
    let Node::Grid(g, tracks, _) = structure::node(&context, &node).unwrap() else {
        panic!()
    };
    let mut demands = [Demand::default(); 32];
    assert!(
        grid::intrinsic(&g, tracks, &demands)
            .unwrap()
            .iter()
            .all(|d| d.minimum == 0 && d.preferred == 0)
    );
    demands[0] = Demand {
        minimum: 51,
        preferred: 201,
        inline_grow: 1,
    };
    let node = ComponentLayoutNode::grid(
        1,
        group(vec![ComponentLayoutChild::new(1, 0, 2, 1, 0).unwrap()]),
        4,
    )
    .unwrap();
    let Node::Grid(g, tracks, _) = structure::node(&context, &node).unwrap() else {
        panic!()
    };
    let tracks = grid::intrinsic(&g, tracks, &demands).unwrap();
    assert_eq!(tracks[0].minimum, 0);
    assert_eq!(tracks[3].minimum, 0);
    assert_eq!(tracks[1].minimum + tracks[2].minimum, 51);
    assert_eq!(tracks[1].preferred + tracks[2].preferred, 201);
    assert!(structure::allocate(&tracks, 50, 0).is_err());
    assert_eq!(
        structure::allocate(&tracks, 301, 0)
            .unwrap()
            .iter()
            .map(|w| u32::from(*w))
            .sum::<u32>(),
        301
    );
}
#[test]
fn grid_conflicts_and_oversized_cells_refuse_without_losing_fallback_identity() {
    let face = super::grid_fixture::face();
    let profile = super::grid_fixture::profile(&face);
    let mut items = vec![item("W", Some(0), true), item("W", Some(1), true)];
    let rows = admit(
        items,
        LayoutRect {
            x: 0,
            y: 0,
            width: 20,
            height: 500,
        },
        &profile,
        100,
        RowSpacing::legacy(SPACE_SM),
        Typography::EDITORIAL,
    );
    assert_eq!(rows.err(), Some(FaceSceneError::InvalidExtent));
    items = (0..35)
        .map(|i| {
            let mut value = item("A bounded control", Some(1), true);
            value.subject_index = Some(i);
            value
        })
        .collect();
    assert_eq!(
        admit(
            items,
            LayoutRect {
                x: 0,
                y: 0,
                width: 900,
                height: 500
            },
            &profile,
            100,
            RowSpacing::legacy(SPACE_SM),
            Typography::EDITORIAL
        )
        .err(),
        Some(FaceSceneError::UnsupportedStructure)
    );
    assert_eq!(profile.face(), &face);
}

#[test]
fn one_spanning_cell_keeps_empty_tracks_at_zero_and_all_original_rows() {
    let face = crate::native_face_scene::tests::face(7);
    let group = |children| {
        ComponentLayoutGroup::new(
            Alignment::Start,
            Break::AccessibleContinuation,
            BoundedSequence::try_from_iter(children).unwrap(),
            0,
            Alignment::Stretch,
            1,
            1,
            0,
            0,
        )
        .unwrap()
    };
    let root = ComponentLayoutNode::grid(
        4,
        group(vec![ComponentLayoutChild::new(2, 1, 2, 1, 1).unwrap()]),
        4,
    )
    .unwrap();
    let content = ComponentLayoutNode::column(
        Alignment::Start,
        Break::AccessibleContinuation,
        BoundedSequence::try_from_iter(
            (2..=4).map(|node| ComponentLayoutChild::new(1, 0, 1, 0, node).unwrap()),
        )
        .unwrap(),
        0,
        Alignment::Stretch,
        1,
        1,
        0,
        0,
    )
    .unwrap();
    let nodes = vec![
        root,
        content,
        ComponentLayoutNode::region(0).unwrap(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::Fallback,
    ];
    let candidate =
        ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0).unwrap();
    let profile = super::tests::prepared(&face, vec![candidate]);
    let rows = super::tests::run(
        vec![
            item("One", Some(0), false),
            item("Two", Some(1), false),
            item("Open meaning", None, false),
        ],
        500,
        500,
        &profile,
    )
    .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(),
        ["One", "Two", "Open meaning"]
    );
    assert!(rows.iter().all(|row| row.bounds.x == 0));
    assert_geometry(&rows, 500, 500);
}

#[test]
fn declared_empty_block_tracks_preserve_internal_and_leading_gaps() {
    let face = crate::native_face_scene::tests::face(7);
    let gap = resolve_em_sixteenths(Typography::EDITORIAL.style(TextRole::Body), 100, 8).unwrap();
    let chrome = chrome::Chrome::prepare_selected(
        LayoutRect {
            x: 0,
            y: 0,
            width: 500,
            height: 500,
        },
        100,
        Typography::EDITORIAL,
    )
    .unwrap();
    for (first, second, empty) in [(0, 2, 1), (1, 2, 0)] {
        let children = [(1, first), (2, second), (3, empty)]
            .map(|(node, block)| ComponentLayoutChild::new(1, block, 1, 0, node).unwrap());
        let group = ComponentLayoutGroup::new(
            Alignment::Start,
            Break::AccessibleContinuation,
            BoundedSequence::try_from_iter(children).unwrap(),
            8,
            Alignment::Stretch,
            1,
            1,
            0,
            0,
        )
        .unwrap();
        let nodes = vec![
            ComponentLayoutNode::grid(3, group, 1).unwrap(),
            ComponentLayoutNode::region(0).unwrap(),
            ComponentLayoutNode::region(1).unwrap(),
            ComponentLayoutNode::Fallback,
        ];
        let candidate =
            ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0)
                .unwrap();
        let profile = super::tests::prepared(&face, vec![candidate]);
        let rows = super::tests::run(
            vec![item("One", Some(0), false), item("Two", Some(1), false)],
            500,
            500,
            &profile,
        )
        .unwrap();
        assert_eq!(rows[0].bounds.y as u16 - chrome.top, gap * u16::from(first));
        assert_eq!(
            rows[1].bounds.y - rows[0].bounds.y,
            rows[0].bounds.height as i16 + (gap * u16::from(second - first)) as i16
        );
        assert_geometry(&rows, 500, 500);
    }
}

#[test]
fn independent_grid_bands_paginate_under_row_and_command_pressure() {
    let face = crate::native_face_scene::tests::face(7);
    let children = (1..=3).map(|node| ComponentLayoutChild::new(1, node - 1, 1, 0, node).unwrap());
    let group = ComponentLayoutGroup::new(
        Alignment::Start,
        Break::AccessibleContinuation,
        BoundedSequence::try_from_iter(children).unwrap(),
        8,
        Alignment::Stretch,
        1,
        1,
        0,
        0,
    )
    .unwrap();
    let nodes = vec![
        ComponentLayoutNode::grid(3, group, 1).unwrap(),
        ComponentLayoutNode::region(0).unwrap(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::Fallback,
    ];
    let candidate =
        ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0).unwrap();
    let profile = super::tests::prepared(&face, vec![candidate]);
    let items = || {
        (0..24)
            .map(|index| {
                item(
                    &format!("Control {index}"),
                    match index / 8 {
                        0 => Some(0),
                        1 => Some(1),
                        _ => None,
                    },
                    true,
                )
            })
            .collect::<Vec<_>>()
    };
    for height in [1000, 2000] {
        let rows = super::tests::run(items(), 1000, height, &profile).unwrap();
        assert_eq!(rows.len(), 24);
        assert_eq!(rows.iter().filter(|row| row.page == 0).count(), 16);
        assert_eq!(rows.iter().filter(|row| row.page == 1).count(), 8);
        let cause = rows.iter().find_map(|row| row.continuation).unwrap();
        assert!(cause.row_limit() && cause.command_limit() && cause.kept_group());
        // The original 1000px specimen also exceeds measured block space.
        // Doubling available height isolates simultaneous resource pressure.
        assert_eq!(cause.block_limit(), height == 1000);
        assert!(
            rows.iter()
                .filter(|row| row.page == 0)
                .all(|row| row.continuation.is_none())
        );
        assert_geometry(&rows, 1000, height);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.text, format!("Control {index}"));
        }
    }
}
#[test]
fn indivisible_grid_band_reports_command_pressure_instead_of_raising_budget() {
    let face = crate::native_face_scene::tests::face(7);
    let children = (1..=3).map(|node| ComponentLayoutChild::new(1, 0, 1, node - 1, node).unwrap());
    let group = ComponentLayoutGroup::new(
        Alignment::Start,
        Break::AccessibleContinuation,
        BoundedSequence::try_from_iter(children).unwrap(),
        0,
        Alignment::Stretch,
        1,
        1,
        0,
        0,
    )
    .unwrap();
    let nodes = vec![
        ComponentLayoutNode::grid(1, group, 3).unwrap(),
        ComponentLayoutNode::region(0).unwrap(),
        ComponentLayoutNode::region(1).unwrap(),
        ComponentLayoutNode::Fallback,
    ];
    let candidate =
        ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0).unwrap();
    let profile = super::tests::prepared(&face, vec![candidate]);
    let items = (0..20)
        .map(|index| item("Control", Some(index / 10), true))
        .collect();
    assert_eq!(
        super::tests::run(items, 1600, 1000, &profile).err(),
        Some(FaceSceneError::SceneBound)
    );
}
