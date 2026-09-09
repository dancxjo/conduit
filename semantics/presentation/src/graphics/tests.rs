use super::*;

#[test]
fn full_scene_round_trips_and_overflow_is_atomic() {
    let command = GraphicsCommand::text(
        rect(0, 10),
        rect(0, 10),
        GraphicsPaintRole::Foreground,
        &"x".repeat(MAX_GRAPHICS_TEXT_BYTES),
    )
    .unwrap();
    let mut scene = GraphicsScene::empty();
    for _ in 0..MAX_GRAPHICS_COMMANDS {
        scene.push(command).unwrap();
    }
    let before = scene;
    assert_eq!(scene.push(command), Err(GraphicsError::TooManyCommands));
    assert_eq!(scene, before);
    assert_eq!(scene.encoded_len(), MAX_GRAPHICS_SCENE_BYTES);
    let mut encoded = scene.encode();
    assert_eq!(GraphicsScene::decode(&encoded), Ok(scene));
    encoded[1] = (MAX_GRAPHICS_COMMANDS + 1) as u8;
    assert_eq!(
        GraphicsScene::decode(&encoded),
        Err(GraphicsError::TooManyCommands)
    );
}

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

#[test]
fn pane_sized_text_is_exactly_bounded_and_round_trips() {
    let exact =
        alloc::string::String::from_utf8(alloc::vec![b'x'; MAX_GRAPHICS_TEXT_BYTES]).unwrap();
    let command = GraphicsCommand::text(
        rect(0, 10),
        rect(0, 10),
        GraphicsPaintRole::Foreground,
        &exact,
    )
    .unwrap();
    assert_eq!(command.payload(), exact);

    let overflow =
        alloc::string::String::from_utf8(alloc::vec![b'x'; MAX_GRAPHICS_TEXT_BYTES + 1]).unwrap();
    assert_eq!(
        GraphicsCommand::text(
            rect(0, 10),
            rect(0, 10),
            GraphicsPaintRole::Foreground,
            &overflow
        ),
        Err(GraphicsError::PayloadTooLong)
    );
}

#[test]
fn full_scene_capacity_round_trips_and_refuses_one_more_command() {
    let command = GraphicsCommand::text(
        rect(0, 10),
        rect(0, 10),
        GraphicsPaintRole::Foreground,
        &"x".repeat(MAX_GRAPHICS_TEXT_BYTES),
    )
    .unwrap();
    let mut scene = GraphicsScene::empty();
    for _ in 0..MAX_GRAPHICS_COMMANDS {
        scene.push(command).unwrap();
    }
    let len = scene.encoded_len();
    let bytes = scene.encode();
    assert_eq!(len, MAX_GRAPHICS_SCENE_BYTES);
    assert_eq!(GraphicsScene::decode(&bytes), Ok(scene));
    assert_eq!(scene.push(command), Err(GraphicsError::TooManyCommands));
}
