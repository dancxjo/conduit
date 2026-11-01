use super::*;

fn rectangle(style: GraphicsShapeStyle) -> GraphicsCommand {
    let bounds = LayoutRect {
        x: 1,
        y: 2,
        width: 40,
        height: 30,
    };
    GraphicsCommand::rect(bounds, bounds, GraphicsPaintRole::Border, style).unwrap()
}

#[test]
fn explicit_geometry_round_trips_every_admitted_pair_without_storage_growth() {
    for style in [
        GraphicsShapeStyle::RoundedFill,
        GraphicsShapeStyle::RoundedStroke,
    ] {
        for radius in 0..=32 {
            for stroke in 1..=8 {
                let legacy = rectangle(style);
                let command = legacy.with_rect_metrics(radius, stroke).unwrap();
                assert_eq!(command.rect_metrics(), Some((radius, stroke)));
                assert_eq!(command.payload(), "");
                assert_eq!(command.bounds, legacy.bounds);
                assert_eq!(command.clip, legacy.clip);
                assert_eq!(
                    core::mem::size_of_val(&command),
                    core::mem::size_of_val(&legacy)
                );
                let mut scene = GraphicsScene::empty();
                scene.push(command).unwrap();
                let bytes = scene.encode();
                assert_eq!(bytes[0], RECT_METRICS_VERSION);
                assert_eq!(scene.encoded_len(), 25);
                assert_eq!(
                    GraphicsScene::decode(&bytes[..scene.encoded_len()]),
                    Ok(scene)
                );
            }
        }
    }
}

#[test]
fn legacy_scenes_keep_exact_version_two_encoding_and_new_geometry_cannot_downgrade() {
    let mut scene = GraphicsScene::empty();
    let legacy = rectangle(GraphicsShapeStyle::RoundedStroke);
    scene.push(legacy).unwrap();
    let bytes = scene.encode();
    assert_eq!(&bytes[..5], &[2, 1, 1, 13, 4]);
    assert_eq!(scene.encoded_len(), 23);
    assert_eq!(bytes[21], 0); // No rectangle payload.
    assert_eq!(GraphicsScene::decode(&bytes[..23]), Ok(scene));
    let mut noncanonical = bytes;
    noncanonical[0] = RECT_METRICS_VERSION;
    assert_eq!(
        GraphicsScene::decode(&noncanonical[..23]),
        Err(GraphicsError::NonCanonicalEncoding)
    );
    let mut styled = GraphicsScene::empty();
    styled
        .push(legacy.with_rect_metrics(2, 3).unwrap())
        .unwrap();
    let mut bytes = styled.encode();
    bytes[0] = VERSION;
    assert_eq!(
        GraphicsScene::decode(&bytes[..styled.encoded_len()]),
        Err(GraphicsError::NonCanonicalEncoding)
    );
}

#[test]
fn malformed_geometry_and_wrong_command_kinds_refuse() {
    for (radius, stroke) in [(33, 1), (0, 0), (32, 9), (255, 255)] {
        assert!(rectangle(GraphicsShapeStyle::RoundedStroke)
            .with_rect_metrics(radius, stroke)
            .is_err());
    }
    for style in [GraphicsShapeStyle::Fill, GraphicsShapeStyle::Stroke] {
        assert!(rectangle(style).with_rect_metrics(0, 1).is_err());
    }
    let bounds = rectangle(GraphicsShapeStyle::Fill).bounds;
    assert!(GraphicsCommand::text(
        bounds,
        bounds,
        GraphicsPaintRole::Foreground,
        "same meaning"
    )
    .unwrap()
    .with_rect_metrics(2, 1)
    .is_err());
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            rectangle(GraphicsShapeStyle::RoundedStroke)
                .with_rect_metrics(2, 1)
                .unwrap(),
        )
        .unwrap();
    let original = scene.encode();
    for (offset, value) in [(23, 33), (24, 0), (24, 9), (21, 1)] {
        let mut bytes = original;
        bytes[offset] = value;
        assert!(GraphicsScene::decode(&bytes[..scene.encoded_len()]).is_err());
    }
}

#[test]
fn explicit_geometry_preserves_the_finite_scene_capacity() {
    let mut scene = GraphicsScene::empty();
    let command = rectangle(GraphicsShapeStyle::RoundedFill)
        .with_rect_metrics(32, 8)
        .unwrap();
    for _ in 0..MAX_GRAPHICS_COMMANDS {
        scene.push(command).unwrap();
    }
    assert_eq!(scene.push(command), Err(GraphicsError::TooManyCommands));
    assert!(scene.encoded_len() <= MAX_GRAPHICS_SCENE_BYTES);
    let bytes = scene.encode();
    assert_eq!(
        GraphicsScene::decode(&bytes[..scene.encoded_len()]),
        Ok(scene)
    );
}

#[test]
fn union_packet_preserves_polyline_value_and_explicit_rectangle_together() {
    use crate::{GraphicsPoint, GraphicsTextRole};
    let bounds = rectangle(GraphicsShapeStyle::Fill).bounds;
    let path = GraphicsPolyline::new(&[
        GraphicsPoint::new(2, 3).unwrap(),
        GraphicsPoint::new(5, 9).unwrap(),
        GraphicsPoint::new(9, 4).unwrap(),
    ])
    .unwrap();
    let mut scene = GraphicsScene::empty();
    scene
        .push(GraphicsCommand::polyline(path, bounds, GraphicsPaintRole::Accent).unwrap())
        .unwrap();
    scene
        .push(
            GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, "19.0 °C")
                .unwrap()
                .with_text_role(GraphicsTextRole::Value)
                .unwrap(),
        )
        .unwrap();
    // Polyline and Value alone still have the version-two packet shape.
    let v2 = scene.encode();
    assert_eq!(v2[0], VERSION);
    assert_eq!(GraphicsScene::decode(&v2[..scene.encoded_len()]), Ok(scene));
    scene
        .push(
            rectangle(GraphicsShapeStyle::RoundedStroke)
                .with_rect_metrics(0, 2)
                .unwrap(),
        )
        .unwrap();
    let v3 = scene.encode();
    assert_eq!(v3[0], RECT_METRICS_VERSION);
    let golden = include_str!("../../../../proof/fixtures/graphics-scene-union.hex").trim();
    let expected: alloc::vec::Vec<u8> = golden
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(&v3[..scene.encoded_len()], expected.as_slice());
    assert_eq!(GraphicsScene::decode(&v3[..scene.encoded_len()]), Ok(scene));
    assert_eq!(MAX_GRAPHICS_COMMANDS, 48);
    assert_eq!(MAX_GRAPHICS_TEXT_BYTES, 192);
    assert_eq!(MAX_GRAPHICS_SCENE_BYTES, 10226);
    assert_eq!(GRAPHICS_SCENE_KIND, "face/graphics-scene@2");
}
