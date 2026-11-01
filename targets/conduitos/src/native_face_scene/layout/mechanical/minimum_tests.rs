//! Authored em minima negotiate actual selected metrics, never viewport breakpoints.
use super::*;
use conduit_plot::rust_binding::BoundedSequence;

#[test]
fn outer_minimum_selects_single_column_and_scales_with_font_metrics() {
    let face = crate::native_face_scene::tests::face(7);
    let wide = tests::wide(false);
    let mut nodes = wide.nodes().as_slice().to_vec();
    let ComponentLayoutNode::Row(group) = &nodes[1] else {
        panic!()
    };
    nodes[1] = ComponentLayoutNode::row(
        *group.block_alignment(),
        *group.break_policy(),
        group.children().clone(),
        *group.gap_em_sixteenths(),
        *group.inline_alignment(),
        *group.inline_grow(),
        *group.inline_shrink(),
        40 * 16,
        *group.padding_em_sixteenths(),
    )
    .unwrap();
    let wide =
        ComponentLayoutCandidate::new(BoundedSequence::try_from_iter(nodes).unwrap(), 0).unwrap();
    let profile = tests::prepared(&face, vec![wide, tests::narrow()]);
    for (width, scale, columns) in [(1000, 100, true), (640, 100, false), (1280, 200, false)] {
        let rows = admit(
            vec![
                tests::item("alpha", Some(0), false),
                tests::item("beta", Some(1), true),
            ],
            LayoutRect {
                x: 0,
                y: 0,
                width,
                height: 900,
            },
            &profile,
            scale,
            RowSpacing::legacy(SPACE_SM),
            Typography::EDITORIAL,
        )
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1].bounds.x > rows[0].bounds.x,
            columns,
            "width={width} scale={scale}"
        );
        tests::assert_geometry(&rows, width, 900);
    }
}
