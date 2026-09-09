use super::*;

#[test]
fn state_paints_round_trip_and_unknown_paints_refuse() {
    for role in [
        GraphicsPaintRole::Muted,
        GraphicsPaintRole::Success,
        GraphicsPaintRole::Warning,
        GraphicsPaintRole::Danger,
        GraphicsPaintRole::Focus,
        GraphicsPaintRole::Hovered,
        GraphicsPaintRole::Selected,
    ] {
        let mut scene = GraphicsScene::empty();
        scene
            .push(GraphicsCommand::text(rect(0, 10), rect(0, 10), role, "meaning").unwrap())
            .unwrap();
        let mut bytes = scene.encode();
        assert_eq!(
            GraphicsScene::decode(&bytes[..scene.encoded_len()]),
            Ok(scene)
        );
        bytes[3] = 255;
        assert_eq!(
            GraphicsScene::decode(&bytes[..scene.encoded_len()]),
            Err(GraphicsError::MalformedEncoding)
        );
    }
}

#[test]
fn graphical_roles_round_trip_and_refuse_unknown_or_nontext_roles() {
    for role in [
        GraphicsTextRole::Body,
        GraphicsTextRole::Label,
        GraphicsTextRole::Heading,
        GraphicsTextRole::Title,
        GraphicsTextRole::Code,
    ] {
        let command = GraphicsCommand::text(
            rect(0, 10),
            rect(0, 10),
            GraphicsPaintRole::Foreground,
            "same text",
        )
        .unwrap()
        .with_text_role(role)
        .unwrap();
        let mut scene = GraphicsScene::empty();
        scene.push(command).unwrap();
        let mut encoded = scene.encode();
        assert_eq!(
            GraphicsScene::decode(&encoded[..scene.encoded_len()]),
            Ok(scene)
        );
        assert_eq!(scene.commands()[0].payload(), "same text");
        encoded[22] = 255;
        assert_eq!(
            GraphicsScene::decode(&encoded[..scene.encoded_len()]),
            Err(GraphicsError::MalformedEncoding)
        );
        encoded[0] = 1;
        assert_eq!(
            GraphicsScene::decode(&encoded[..scene.encoded_len()]),
            Err(GraphicsError::MalformedEncoding)
        );
    }
    assert!(GraphicsCommand::rect(
        rect(0, 10),
        rect(0, 10),
        GraphicsPaintRole::Background,
        GraphicsShapeStyle::Fill
    )
    .unwrap()
    .with_text_role(GraphicsTextRole::Code)
    .is_err());
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
