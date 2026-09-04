use super::*;
fn rect(x: i16, width: u16) -> LayoutRect {
    LayoutRect {
        x,
        y: 0,
        width,
        height: 10,
    }
}

#[test]
fn round_trip_and_clip_classes_are_raster_independent() {
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                rect(0, 10),
                rect(0, 10),
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .unwrap(),
        )
        .unwrap();
    scene
        .push(
            GraphicsCommand::text(
                rect(5, 10),
                rect(0, 10),
                GraphicsPaintRole::Foreground,
                "ready",
            )
            .unwrap(),
        )
        .unwrap();
    scene
        .push(
            GraphicsCommand::icon(
                rect(20, 5),
                rect(0, 10),
                GraphicsPaintRole::Accent,
                PresentationIconKey::Presentation,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        scene
            .commands()
            .iter()
            .map(GraphicsCommand::clip_class)
            .collect::<alloc::vec::Vec<_>>(),
        alloc::vec![
            GraphicsClipClass::FullyVisible,
            GraphicsClipClass::PartiallyClipped,
            GraphicsClipClass::FullyClipped
        ]
    );
    let encoded = scene.encode();
    assert_eq!(
        GraphicsScene::decode(&encoded[..scene.encoded_len()]),
        Ok(scene)
    );
}

#[test]
fn malformed_overflow_and_unknown_icon_refuse() {
    assert_eq!(
        GraphicsCommand::text(rect(0, 0), rect(0, 1), GraphicsPaintRole::Foreground, "x"),
        Err(GraphicsError::InvalidGeometry)
    );
    assert_eq!(
        GraphicsCommand::new(
            GraphicsCommandKind::Icon,
            rect(0, 1),
            rect(0, 1),
            GraphicsPaintRole::Accent,
            GraphicsShapeStyle::Fill,
            b"invented"
        ),
        Err(GraphicsError::UnknownIcon)
    );
    let mut encoded = [0; 3];
    encoded[0] = VERSION;
    assert_eq!(
        GraphicsScene::decode(&encoded),
        Err(GraphicsError::NonCanonicalEncoding)
    );
}
